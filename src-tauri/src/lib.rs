mod config;
mod error;
mod event;
mod http;
mod logging;
mod login_item;
mod presentation;
mod queue;

use std::sync::{Arc, Once, OnceLock};
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::webview::PageLoadEvent;
use tauri::{ActivationPolicy, Manager};
use tokio::sync::Mutex;

use crate::config::Config;
use crate::event::emit_promoted;
use crate::queue::NotificationQueue;

// tracing-appender flushes through this guard; it must live as long as
// the process, so it's parked in a static rather than dropped at the
// end of run()'s setup.
static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

pub fn run() {
    match logging::init_logging() {
        Ok(guard) => {
            let _ = LOG_GUARD.set(guard);
        }
        Err(e) => eprintln!("notchtap: file logging unavailable: {e}"),
    }

    // malformed config is a boot-time error: fail fast with a clear
    // message (spec §9). a missing file is fine and yields defaults.
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("{e}");
            eprintln!("notchtap: {e}");
            std::process::exit(1);
        }
    };

    let (mode, inset) = presentation::detect_mode(&config);
    // this info line is load-bearing: the hud fallback is silent by
    // design, so the log is the only tell that detection worked
    // (manual checklist, IMPLEMENTATION_PLAN.md §5)
    tracing::info!(?mode, inset, "presentation mode resolved");

    let queue = Arc::new(Mutex::new(NotificationQueue::new(
        config.max_concurrent,
        config.max_queued,
    )));
    let port = config.port;
    let default_ttl = config.default_ttl;

    let setup_queue = queue.clone();
    let page_load_queue = queue.clone();
    let server_once = Arc::new(Once::new());

    tauri::Builder::default()
        .setup(move |app| {
            app.set_activation_policy(ActivationPolicy::Accessory);

            let window = app
                .get_webview_window("main")
                .expect("main window missing from tauri.conf.json");
            window.set_always_on_top(true)?;
            position_top_center(&window)?;

            login_item::register();
            build_tray(app.handle(), setup_queue.clone())?;
            spawn_heartbeat(app.handle().clone(), setup_queue);

            Ok(())
        })
        .on_page_load(move |webview, payload| {
            // listener-ready gate (spec §3): tauri events are transient, so
            // the /notify listener binds only once the webview has loaded
            // and its `notification-promoted` listener can exist. before
            // this, the cli gets connection-refused — honest, not a silent
            // 200-drop.
            if payload.event() == PageLoadEvent::Finished && webview.label() == "main" {
                let queue = page_load_queue.clone();
                let app_handle = webview.app_handle().clone();
                server_once.call_once(move || {
                    let state = http::AppState {
                        queue,
                        default_ttl,
                        app_handle,
                    };
                    tauri::async_runtime::spawn(async move {
                        let listener = match http::bind_listener(port).await {
                            Ok(l) => l,
                            Err(e) => {
                                // ARCHITECTURE.md §7: a taken port is a hard
                                // startup error, never a silent fallback port
                                tracing::error!("cannot bind 127.0.0.1:{port}: {e}");
                                eprintln!("notchtap: cannot bind 127.0.0.1:{port}: {e}");
                                std::process::exit(1);
                            }
                        };
                        tracing::info!("listening on 127.0.0.1:{port}");
                        if let Err(e) = axum::serve(listener, http::router(state)).await {
                            tracing::error!("http server exited: {e}");
                        }
                    });
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running notchtap");
}

fn position_top_center(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    if let Some(monitor) = window.current_monitor()? {
        let screen = monitor.size();
        let win = window.outer_size()?;
        let x = (screen.width as i32 - win.width as i32) / 2;
        window.set_position(tauri::PhysicalPosition::new(x, 0))?;
    }
    Ok(())
}

fn build_tray(
    app: &tauri::AppHandle,
    queue: Arc<Mutex<NotificationQueue>>,
) -> tauri::Result<()> {
    let pause_item = MenuItem::with_id(app, "pause", "Pause", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&pause_item, &quit_item])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "pause" => {
                // menu events arrive on the main thread, outside the tokio
                // runtime, so a blocking lock is safe here
                let promoted = {
                    let mut q = queue.blocking_lock();
                    if q.is_paused() {
                        q.resume();
                        // spec §4: resume promotes immediately, not on the
                        // next heartbeat tick
                        q.expire_and_promote(Instant::now());
                        let _ = pause_item.set_text("Pause");
                        q.take_promoted()
                    } else {
                        q.pause();
                        let _ = pause_item.set_text("Resume");
                        Vec::new()
                    }
                };
                emit_promoted(app, promoted);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn spawn_heartbeat(app: tauri::AppHandle, queue: Arc<Mutex<NotificationQueue>>) {
    // 250ms promotion heartbeat (spec §4): expiry and promotion never
    // depend on a new push arriving
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            interval.tick().await;
            let promoted = {
                let mut q = queue.lock().await;
                q.expire_and_promote(Instant::now());
                q.take_promoted()
            };
            emit_promoted(&app, promoted);
        }
    });
}

