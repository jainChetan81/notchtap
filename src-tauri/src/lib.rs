mod config;
// queue, event, and error are `pub` so their doc-tests can exercise the
// real public api (doc-tests link against the lib crate from outside);
// nothing else consumes this crate as a library.
pub mod error;
pub mod event;
mod http;
mod logging;
mod login_item;
mod notifier;
mod poller;
mod presentation;
pub mod queue;

use std::sync::atomic::{AtomicBool, Ordering};
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

    let (mode, inset, cutout) = presentation::detect_mode(&config);
    // this info line is load-bearing: the hud fallback is silent by
    // design, so the log is the only tell that detection worked
    // (manual checklist, IMPLEMENTATION_PLAN.md §6)
    tracing::info!(?mode, inset, "presentation mode resolved");

    let queue = Arc::new(Mutex::new(NotificationQueue::new(
        config.max_concurrent,
        config.max_queued,
    )));
    let port = config.port;
    let default_ttl = config.default_ttl;
    let espn_enabled = config.espn_enabled;
    let espn_leagues = config.espn_leagues.clone();
    let espn_poll_secs = config.espn_poll_secs;

    // v3 outbound connectors: built here (channel needs no runtime), the
    // worker future is spawned in setup once the runtime exists. missing
    // or badly-permissioned secrets disable the connector with a warning —
    // the app runs overlay-only (v3 spec §4).
    let mut connector_handles = Vec::new();
    let mut telegram_worker = None;
    if config.connectors.telegram.enabled {
        match notifier::default_secrets_path() {
            Some(path) => match notifier::load_secrets(&path) {
                Ok(secrets) => {
                    let (handle, worker) = notifier::telegram_connector(
                        secrets,
                        notifier::TELEGRAM_API_BASE.to_string(),
                        notifier::RETRY_DELAY,
                    );
                    connector_handles.push(handle);
                    telegram_worker = Some(worker);
                    tracing::info!("telegram connector enabled");
                }
                Err(e) => tracing::warn!("telegram connector disabled: {e}"),
            },
            None => tracing::warn!("telegram connector disabled: no home directory"),
        }
    }
    let connectors = Arc::new(connector_handles);
    let page_load_connectors = connectors.clone();
    let poller_connectors = connectors.clone();

    let setup_queue = queue.clone();
    let page_load_queue = queue.clone();
    let server_once = Arc::new(Once::new());
    let mode_once = Arc::new(Once::new());

    tauri::Builder::default()
        .setup(move |app| {
            app.set_activation_policy(ActivationPolicy::Accessory);

            let window = app
                .get_webview_window("main")
                .expect("main window missing from tauri.conf.json");
            window.set_always_on_top(true)?;
            position_window(&window, mode, cutout)?;

            login_item::register();
            if let Some(worker) = telegram_worker {
                tauri::async_runtime::spawn(worker);
            }
            // tray-controlled polling switch: true = polling. lives here
            // (not in the queue) because it gates network fetches, not
            // promotion — pausing scores must not touch cmux/cli pushes.
            let espn_active = espn_enabled.then(|| Arc::new(AtomicBool::new(true)));
            build_tray(app.handle(), setup_queue.clone(), espn_active.clone())?;
            spawn_heartbeat(app.handle().clone(), setup_queue.clone());

            // espn poller (v2 spec §3) — config-gated: `espn_enabled =
            // false` means it never spawns. first poll only baselines
            // (silent), so starting before the webview loads can't drop
            // anything a listener would have shown.
            if let Some(espn_active) = espn_active {
                poller::spawn_espn_poller(
                    app.handle().clone(),
                    setup_queue,
                    poller_connectors,
                    espn_leagues,
                    espn_poll_secs,
                    default_ttl,
                    espn_active,
                );
            }

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
                let connectors = page_load_connectors.clone();

                mode_once.call_once(|| {
                    use tauri::Emitter;
                    let _ = webview.emit("presentation-mode", PresentationModePayload { mode });
                });

                server_once.call_once(move || {
                    let app_handle = app_handle.clone();
                    let state = http::AppState {
                        queue,
                        default_ttl,
                        app_handle: app_handle.clone(),
                        connectors,
                    };
                    tauri::async_runtime::spawn(async move {
                        let listener = match http::bind_listener(port).await {
                            Ok(l) => l,
                            Err(e) => {
                                // ARCHITECTURE.md §7: a taken port is a hard
                                // startup error, never a silent fallback port
                                tracing::error!("cannot bind 127.0.0.1:{port}: {e}");
                                eprintln!("notchtap: cannot bind 127.0.0.1:{port}: {e}");
                                app_handle.exit(1);
                                return;
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

// notch-morph nudge (plan §3.5): anchor to the reported cutout when we have
// notch-precise geometry, else fall back to screen-center (covers hud mode,
// and notch mode when the shim couldn't report a cutout).
fn position_window(
    window: &tauri::WebviewWindow,
    mode: presentation::Mode,
    cutout: Option<presentation::CutoutGeometry>,
) -> tauri::Result<()> {
    if let (presentation::Mode::Notch, Some(cutout)) = (mode, cutout) {
        let scale_factor = window.scale_factor()?;
        let win_width = window.outer_size()?.to_logical::<f64>(scale_factor).width;
        let x = cutout.center_x() - (win_width / 2.0);
        window.set_position(tauri::LogicalPosition::new(x, 0.0))?;
        Ok(())
    } else {
        position_top_center(window)
    }
}

#[derive(serde::Serialize, Clone)]
struct PresentationModePayload {
    mode: presentation::Mode,
}

fn build_tray(
    app: &tauri::AppHandle,
    queue: Arc<Mutex<NotificationQueue>>,
    espn_active: Option<Arc<AtomicBool>>,
) -> tauri::Result<()> {
    let pause_item = MenuItem::with_id(app, "pause", "Pause", true, None::<&str>)?;
    // only offered when the poller exists (`espn_enabled = true`)
    let espn_item = espn_active
        .as_ref()
        .map(|_| MenuItem::with_id(app, "espn", "Pause Football Scores", true, None::<&str>))
        .transpose()?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::new(app)?;
    menu.append(&pause_item)?;
    if let Some(item) = &espn_item {
        menu.append(item)?;
    }
    menu.append(&quit_item)?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "pause" => {
                // menu events arrive on the main thread, outside the tokio
                // runtime, so a blocking lock is safe here
                debug_assert!(
                    tokio::runtime::Handle::try_current().is_err(),
                    "tray menu events must arrive off the tokio runtime; blocking_lock would deadlock"
                );
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
            "espn" => {
                if let (Some(flag), Some(item)) = (&espn_active, &espn_item) {
                    // fetch_xor toggles and returns the previous value
                    let now_active = !flag.fetch_xor(true, Ordering::Relaxed);
                    let _ = item.set_text(if now_active {
                        "Pause Football Scores"
                    } else {
                        "Resume Football Scores"
                    });
                    tracing::info!(active = now_active, "espn polling toggled from tray");
                }
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
