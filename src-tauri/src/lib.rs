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
use crate::event::emit_slot_state;
use crate::queue::SingleSlotQueue;

#[cfg(target_os = "macos")]
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

// placeholder combo — v3.6 spec §7.1 explicitly defers "exact global hotkey
// combination" as an open detail; isolated to one constant.
#[cfg(target_os = "macos")]
const EXPAND_TOGGLE_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyN);

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

    let queue = Arc::new(Mutex::new(SingleSlotQueue::new(config.max_queued_per_tier)));
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
    #[cfg(target_os = "macos")]
    let hotkey_queue = queue.clone();
    let server_once = Arc::new(Once::new());

    tauri::Builder::default()
        .setup(move |app| {
            app.set_activation_policy(ActivationPolicy::Accessory);

            let window = app
                .get_webview_window("main")
                .expect("main window missing from tauri.conf.json");
            window.set_always_on_top(true)?;

            // v3.6 spec §7.2: survive Spaces switches and fullscreen apps.
            #[cfg(target_os = "macos")]
            {
                use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
                let ns_window_ptr = window.ns_window()? as *mut NSWindow;
                let ns_window: &NSWindow = unsafe { &*ns_window_ptr };
                let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary;
                ns_window.setCollectionBehavior(behavior);
            }

            position_window(&window, mode, cutout)?;

            // v3.6 spec §7.1: manual expand toggle, rust-side only — the
            // frontend never calls the plugin's JS api (receive-only
            // boundary, unchanged), so no capabilities/permissions entry
            // is needed for this.
            #[cfg(target_os = "macos")]
            {
                let hotkey_queue_for_handler = hotkey_queue.clone();
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |app, _shortcut, event| {
                            if event.state() == ShortcutState::Pressed {
                                toggle_manual_expand(app, &hotkey_queue_for_handler);
                            }
                        })
                        .build(),
                )?;
                app.global_shortcut().register(Shortcut::new(
                    EXPAND_TOGGLE_SHORTCUT.0,
                    EXPAND_TOGGLE_SHORTCUT.1,
                ))?;
            }

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

                // mode delivery is double-shielded against the
                // listener-registration race (2026-07-17 review): the eval
                // plants a global that react reads as *initial* state if it
                // mounts after this moment; the emit reaches the listener if
                // react mounted before it. one of the two always lands, and
                // running on every page load (not once) covers reloads too.
                {
                    use tauri::Emitter;
                    let mode_str = match mode {
                        presentation::Mode::Notch => "notch",
                        presentation::Mode::Hud => "hud",
                    };
                    let _ = webview.eval(format!("window.__NOTCHTAP_MODE__ = '{mode_str}';"));
                    let _ = webview.emit("presentation-mode", PresentationModePayload { mode });
                }

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

        // coordinate-space invariant (2026-07-17 review): NSScreen reports
        // points (= logical px, global origin); tauri's LogicalPosition
        // shares the x-axis on the primary display. multi-display
        // arrangements can break that assumption, so a result outside the
        // current monitor falls back to top-center instead of placing the
        // window somewhere invisible. y stays 0.0 deliberately: the cards
        // sit flush with the screen top, inside the notch band.
        if let Some(monitor) = window.current_monitor()? {
            let m_pos = monitor.position().to_logical::<f64>(scale_factor);
            let m_size = monitor.size().to_logical::<f64>(scale_factor);
            if x < m_pos.x || (x + win_width) > (m_pos.x + m_size.width) {
                tracing::warn!(
                    x,
                    "cutout-anchored x lands outside the current monitor; falling back to top-center"
                );
                return position_top_center(window);
            }
        }

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
    queue: Arc<Mutex<SingleSlotQueue>>,
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
                let slot_change = {
                    let mut q = queue.blocking_lock();
                    if q.is_paused() {
                        q.resume();
                        // v3.6 spec §4.5: resume promotes immediately, not
                        // on the next heartbeat tick
                        q.tick(Instant::now());
                        let _ = pause_item.set_text("Pause");
                        q.slot_state_if_changed()
                    } else {
                        q.pause();
                        let _ = pause_item.set_text("Resume");
                        None
                    }
                };
                if let Some(state) = slot_change {
                    emit_slot_state(app, state);
                }
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

fn spawn_heartbeat(app: tauri::AppHandle, queue: Arc<Mutex<SingleSlotQueue>>) {
    // 250ms rotation heartbeat (v3.6 spec §4.3): rotation and promotion
    // never depend on a new push arriving
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            interval.tick().await;
            let slot_change = {
                let mut q = queue.lock().await;
                q.tick(Instant::now());
                q.slot_state_if_changed()
            };
            if let Some(state) = slot_change {
                emit_slot_state(&app, state);
            }
        }
    });
}

// v3.6 spec §7.1.1: the hotkey is a manual override for "everything else"
// (§3.6's wording) — it's a no-op while the current slot is already
// auto-expanded (High priority), not a forced-collapse of an automatic
// expand.
#[cfg(target_os = "macos")]
fn toggle_manual_expand<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    queue: &Arc<Mutex<SingleSlotQueue>>,
) {
    let mut q = queue.blocking_lock();
    if q.current_priority() == Some(crate::event::Priority::High) {
        return;
    }
    q.toggle_expanded();
    if let Some(state) = q.slot_state_if_changed() {
        drop(q);
        emit_slot_state(app, state);
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::event::{Event, EventPayload, EventType, Priority, RotationSpec, SlotState};

    fn event(priority: Priority) -> Event {
        Event {
            id: uuid::Uuid::new_v4(),
            event_type: EventType::Generic,
            priority,
            rotation: RotationSpec::OneShot { ttl_secs: 8 },
            topic: None,
            payload: EventPayload {
                title: "t".to_string(),
                body: "b".to_string(),
            },
        }
    }

    #[test]
    fn toggle_manual_expand_is_a_noop_while_high_priority_is_visible() {
        let app = tauri::test::mock_app();
        let mut inner = SingleSlotQueue::new(50);
        inner.enqueue(event(Priority::High)).unwrap();
        let queue = Arc::new(Mutex::new(inner));

        toggle_manual_expand(&app.handle().clone(), &queue);

        let q = queue.blocking_lock();
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => assert!(!expanded, "High must not toggle"),
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn toggle_manual_expand_flips_expanded_for_non_high_priority() {
        let app = tauri::test::mock_app();
        let mut inner = SingleSlotQueue::new(50);
        inner.enqueue(event(Priority::Medium)).unwrap();
        let queue = Arc::new(Mutex::new(inner));

        toggle_manual_expand(&app.handle().clone(), &queue);

        let q = queue.blocking_lock();
        match q.current_slot_state() {
            SlotState::Showing { expanded, .. } => assert!(expanded, "Medium must toggle"),
            SlotState::Empty => panic!("expected Showing"),
        }
    }
}
