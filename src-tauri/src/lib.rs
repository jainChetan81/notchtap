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
mod rss_poller;
mod settings;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex as StdMutex, Once, OnceLock};
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::webview::PageLoadEvent;
use tauri::{ActivationPolicy, Manager};
use tokio::sync::Mutex;

use crate::config::Config;
use crate::event::emit_slot_state;
use crate::queue::SingleSlotQueue;
use crate::settings::AppearanceChangedPayload;

#[cfg(target_os = "macos")]
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

// placeholder combo — v3.6 spec §7.1 explicitly defers "exact global hotkey
// combination" as an open detail; isolated to one constant.
#[cfg(target_os = "macos")]
const EXPAND_TOGGLE_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyN);
#[cfg(target_os = "macos")]
const OPEN_STORY_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyO);
#[cfg(target_os = "macos")]
const DISMISS_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyX);
#[cfg(target_os = "macos")]
const PAUSE_TOGGLE_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyP);

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

    // v5 kill switch (spec §5): launch with promotion already paused.
    // reuses the paused semantics wholesale — pushes still buffer (202),
    // rotation still ages out anything visible; only the launch state
    // differs. the tray toggle stays session-only.
    let mut initial_queue = SingleSlotQueue::new(config.max_queued_per_tier)
        .with_rotation_order(config.rotation_order.clone());
    if config.start_paused {
        initial_queue.pause();
        tracing::info!("start_paused: launching with promotion paused");
    }
    let queue = Arc::new(Mutex::new(initial_queue));
    let start_paused = config.start_paused;
    // v5 settings window reads the *booted* config via get_config —
    // managed as state in setup, after the fields below are cloned out.
    let config_for_state = config.clone();
    let port = config.port;
    let default_ttl = config.default_ttl;
    let espn_enabled = config.espn_enabled;
    let espn_leagues = config.espn_leagues.clone();
    let espn_poll_secs = config.espn_poll_secs;
    let espn_priority = config.espn_priority;
    let espn_ttl_secs = config.espn_ttl_secs;
    let rss_enabled = config.rss_enabled;
    let rss_feeds = config.rss_feeds.clone();
    let rss_poll_secs = config.rss_poll_secs;
    let rss_priority = config.rss_priority;
    let rss_ttl_secs = config.rss_ttl_secs;
    let rss_max_per_poll = config.rss_max_per_poll;
    let manual_default_priority = config.manual_default_priority;
    let cmux_priority = config.cmux_priority;
    let cmux_ttl_secs = config.cmux_ttl_secs;

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
    let rss_queue = queue.clone();
    let page_load_queue = queue.clone();
    #[cfg(target_os = "macos")]
    let hotkey_queue = queue.clone();
    let server_once = Arc::new(Once::new());

    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        // v5 settings commands (settings.rs) — every one of these is also
        // listed in build.rs's AppManifest::commands; that pairing is what
        // keeps them deniable to the overlay window (spec §2).
        .invoke_handler(tauri::generate_handler![
            settings::get_config,
            settings::get_secret_status,
            settings::save_config_and_relaunch,
            settings::set_secret,
            settings::send_test_notification,
            settings::set_appearance,
        ])
        .setup(move |app| {
            app.set_activation_policy(ActivationPolicy::Accessory);
            app.manage(StdMutex::new(config_for_state));
            // These are also cloned into the /notify handler and pollers,
            // but publishing them as managed state lets the settings commands
            // enqueue test notifications through the same queue/connectors.
            app.manage(queue.clone());
            app.manage(connectors.clone());

            let window = app
                .get_webview_window("main")
                .expect("main window missing from tauri.conf.json");
            window.set_always_on_top(true)?;

            // permanent-overlay pass: a plain NSWindow is never composited
            // into another app's fullscreen Space, regardless of level or
            // collection behavior — macOS only honors fullScreenAuxiliary
            // for nonactivating panels (or perfectly nonactivating agent
            // windows, which tao's show path is not). swizzle the window
            // into an NSPanel with the nonactivating style mask; same
            // object, so all other window APIs keep working.
            #[cfg(target_os = "macos")]
            {
                use tauri_nspanel::WebviewWindowExt as _;
                let panel = window
                    .to_panel()
                    .map_err(|e| format!("nspanel conversion failed: {e:?}"))?;
                // NSWindowStyleMaskNonactivatingPanel (1 << 7); the window
                // is borderless (mask 0), so the panel bit is the whole mask.
                panel.set_style_mask(1 << 7);
            }

            // v3.6 spec §7.2: survive Spaces switches and fullscreen apps.
            #[cfg(target_os = "macos")]
            apply_overlay_native_config(&window)?;

            position_window(&window, mode, cutout)?;
            let pause_item = build_tray(app.handle(), setup_queue.clone(), start_paused)?;

            // v3.6 spec §7.1: manual expand toggle, rust-side only — the
            // frontend never calls the plugin's JS api (receive-only
            // boundary, unchanged), so no capabilities/permissions entry
            // is needed for this.
            #[cfg(target_os = "macos")]
            {
                let hotkey_queue_for_handler = hotkey_queue.clone();
                let pause_item_for_handler = pause_item.clone();
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |app, shortcut, event| {
                            if event.state() == ShortcutState::Pressed {
                                if *shortcut
                                    == Shortcut::new(
                                        EXPAND_TOGGLE_SHORTCUT.0,
                                        EXPAND_TOGGLE_SHORTCUT.1,
                                    )
                                {
                                    toggle_manual_expand(app, &hotkey_queue_for_handler);
                                } else if *shortcut
                                    == Shortcut::new(OPEN_STORY_SHORTCUT.0, OPEN_STORY_SHORTCUT.1)
                                {
                                    open_current_story(app, &hotkey_queue_for_handler);
                                } else if *shortcut
                                    == Shortcut::new(DISMISS_SHORTCUT.0, DISMISS_SHORTCUT.1)
                                {
                                    dismiss_current(app, &hotkey_queue_for_handler);
                                } else if *shortcut
                                    == Shortcut::new(
                                        PAUSE_TOGGLE_SHORTCUT.0,
                                        PAUSE_TOGGLE_SHORTCUT.1,
                                    )
                                {
                                    toggle_pause(
                                        app,
                                        &hotkey_queue_for_handler,
                                        &pause_item_for_handler,
                                    );
                                }
                            }
                        })
                        .build(),
                )?;
                app.global_shortcut().register(Shortcut::new(
                    EXPAND_TOGGLE_SHORTCUT.0,
                    EXPAND_TOGGLE_SHORTCUT.1,
                ))?;
                app.global_shortcut()
                    .register(Shortcut::new(OPEN_STORY_SHORTCUT.0, OPEN_STORY_SHORTCUT.1))?;
                app.global_shortcut()
                    .register(Shortcut::new(DISMISS_SHORTCUT.0, DISMISS_SHORTCUT.1))?;
                app.global_shortcut().register(Shortcut::new(
                    PAUSE_TOGGLE_SHORTCUT.0,
                    PAUSE_TOGGLE_SHORTCUT.1,
                ))?;
            }

            login_item::register();
            if let Some(worker) = telegram_worker {
                tauri::async_runtime::spawn(worker);
            }
            // Poll-loop gate: true = polling. Lives here (not in the queue)
            // because it gates network fetches, not promotion — pausing
            // scores must not touch cmux/cli pushes. v6: no longer
            // tray-toggleable (the tray's "Pause Football Scores"/"Pause
            // News" items were redundant with the settings panel's
            // espn_enabled/rss_enabled toggles, ARCHITECTURE.md §17's
            // "richer than a toggle lives in Settings" rule) — set once at
            // boot from Config and never flipped again.
            let espn_active = espn_enabled.then(|| Arc::new(AtomicBool::new(true)));
            let rss_active = rss_enabled.then(|| Arc::new(AtomicBool::new(true)));
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
                    espn_ttl_secs,
                    espn_priority,
                    espn_active,
                );
            }
            if let Some(rss_active) = rss_active {
                rss_poller::spawn_rss_poller(
                    app.handle().clone(),
                    rss_queue,
                    rss_feeds,
                    rss_poll_secs,
                    rss_ttl_secs,
                    rss_max_per_poll,
                    rss_priority,
                    rss_active,
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
                let eval_queue = page_load_queue.clone();
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

                // same double-shield, mirrored for `slot-state` (this
                // migration's own fix): the transient-event race the
                // presentation-mode shield above was already built for
                // also affects slot-state, but was never applied there —
                // an independent review caught this. blocking_lock is
                // safe here, same as the tray menu handler below: this
                // callback runs off the tokio runtime, not on it.
                {
                    let current_state = eval_queue.blocking_lock().current_slot_state();
                    let state_json =
                        serde_json::to_string(&current_state).unwrap_or_else(|_| "null".into());
                    // unlike presentation-mode's fixed enum, this payload is
                    // arbitrary caller text (espn scoring-play strings, cmux
                    // titles) — escape everything that's illegal or unsafe
                    // to splice raw into eval'd script text: U+2028/U+2029
                    // are legal in JSON strings but illegal raw in JS
                    // source, and `<` closes the gap JSON encoding leaves
                    // open (it doesn't escape `/`, so a literal "</script>"
                    // in a title would otherwise break out of this context).
                    let safe_json = state_json
                        .replace('\u{2028}', "\\u2028")
                        .replace('\u{2029}', "\\u2029")
                        .replace('<', "\\u003c");
                    let _ = webview.eval(format!("window.__NOTCHTAP_SLOT_STATE__ = {safe_json};"));
                    emit_slot_state(&app_handle, current_state);
                }

                // Double-shield the initial appearance values the same way as
                // presentation mode / slot state: a global for the React mount
                // race, plus an emit for listeners already registered.
                {
                    use tauri::Emitter;
                    let appearance = app_handle
                        .state::<StdMutex<Config>>()
                        .lock()
                        .unwrap()
                        .appearance
                        .clone();
                    let payload = AppearanceChangedPayload::from(&appearance);
                    let payload_json = serde_json::to_string(&payload)
                        .unwrap_or_else(|_| "null".into())
                        .replace('\u{2028}', "\\u2028")
                        .replace('\u{2029}', "\\u2029")
                        .replace('<', "\\u003c");
                    let _ = webview.eval(format!(
                        "window.__NOTCHTAP_APPEARANCE__ = {payload_json};"
                    ));
                    let _ = webview.emit("appearance-changed", &payload);
                }

                // re-assert level/collection-behavior/position now that the
                // window is shown — tao's show path resets them (see
                // apply_overlay_native_config).
                #[cfg(target_os = "macos")]
                if let Some(window) = app_handle.get_webview_window("main") {
                    let w = window.clone();
                    let _ = window.run_on_main_thread(move || {
                        if let Err(e) = apply_overlay_native_config(&w) {
                            tracing::warn!("overlay native config re-apply failed: {e}");
                        }
                        if let Err(e) = position_window(&w, mode, cutout) {
                            tracing::warn!("overlay re-position failed: {e}");
                        }
                    });
                }

                server_once.call_once(move || {
                    let app_handle = app_handle.clone();
                    let state = http::AppState {
                        queue,
                        default_ttl,
                        manual_default_priority,
                        cmux_priority,
                        cmux_ttl_secs,
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

// v3.6 spec §7.2 + permanent-overlay pass: the window must overlap the menu
// bar (flush to y=0), survive Spaces switches, and stay visible over
// fullscreen apps. tao resets the window level and collection behavior when
// it shows the window (observed live: layer back to 5, y clamped below the
// menu bar), so this must be applied both at setup AND re-applied after the
// window is actually shown (the page-load hook).
#[cfg(target_os = "macos")]
fn apply_overlay_native_config(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2_app_kit::{NSStatusWindowLevel, NSWindow, NSWindowCollectionBehavior};
    // click-through, always (2026-07-17 bug: on notchless HUD-mode machines,
    // the flush-to-top/NSStatusWindowLevel placement below lands this window
    // directly over the real, interactive system menu bar — not a notch
    // cutout's dead zone — so without this, every click in its bounds
    // (including ones meant for the menu bar's own tray icons) was captured
    // by notchtap instead of passing through. safe unconditionally: the
    // frontend is receive-only and has no click handlers anywhere — every
    // interaction is a global hotkey (⌃⇧N/⌃⇧O), never a click.
    window.set_ignore_cursor_events(true)?;
    // tao tracks this flag in its own window state, so it survives tao's
    // internal re-applies (unlike a raw setCollectionBehavior alone).
    window.set_visible_on_all_workspaces(true)?;
    let ns_window_ptr = window.ns_window()? as *mut NSWindow;
    let ns_window: &NSWindow = unsafe { &*ns_window_ptr };
    // set the EXACT behavior, never OR with the current bits: tao puts
    // FullScreenNone on non-resizable windows, and that bit silently defeats
    // FullScreenAuxiliary (the window then never joins fullscreen Spaces).
    // Stationary + IgnoresCycle make it behave like a system overlay
    // (unaffected by Exposé, skipped by cmd-backtick cycling).
    let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::Stationary
        | NSWindowCollectionBehavior::IgnoresCycle;
    ns_window.setCollectionBehavior(behavior);
    // Floating-tier levels cannot overlap the menu bar or appear over
    // fullscreen Spaces; status level (25) can — required for the
    // flush-to-top permanent overlay.
    ns_window.setLevel(NSStatusWindowLevel);
    tracing::info!(
        behavior = ns_window.collectionBehavior().0,
        level = ns_window.level(),
        "overlay native config applied"
    );
    Ok(())
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

fn toggle_pause<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    queue: &Arc<Mutex<SingleSlotQueue>>,
    pause_item: &MenuItem<R>,
) {
    // menu events and global-shortcut handlers arrive on the main thread,
    // outside the tokio runtime, so a blocking lock is safe here
    debug_assert!(
        tokio::runtime::Handle::try_current().is_err(),
        "pause toggles must arrive off the tokio runtime; blocking_lock would deadlock"
    );
    let slot_change = {
        let mut q = queue.blocking_lock();
        if q.is_paused() {
            q.resume();
            // v3.6 spec §4.5: resume promotes immediately, not on the next
            // heartbeat tick
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

/// v6: the tray is deliberately minimal — Pause/Resume, Settings…, Quit.
/// It previously also carried "Pause Football Scores"/"Pause News" items,
/// but those duplicated the `espn_enabled`/`rss_enabled` toggles already in
/// Settings (which, since v6, also carry per-source priority and rotation
/// order — richer than a toggle belongs there, per ARCHITECTURE.md §17's
/// "everything richer than a toggle lives [in Settings], not in more tray
/// items" rule, which this tray had not yet caught up to).
fn build_tray<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    queue: Arc<Mutex<SingleSlotQueue>>,
    start_paused: bool,
) -> tauri::Result<MenuItem<R>> {
    // v5 kill switch: a start_paused boot renders the toggle as "Resume"
    // from the first open — the label always names the *next* action.
    let initial_pause_label = if start_paused { "Resume" } else { "Pause" };
    let pause_item = MenuItem::with_id(app, "pause", initial_pause_label, true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let pause_item_for_handler = pause_item.clone();
    let menu = Menu::new(app)?;
    menu.append(&pause_item)?;
    menu.append(&settings_item)?;
    menu.append(&quit_item)?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "pause" => toggle_pause(app, &queue, &pause_item_for_handler),
            "settings" => open_settings_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(pause_item)
}

/// v5 spec §1: lazy creation, focus-if-open. A normal decorated window —
/// everything the overlay is not (no nspanel, no always-on-top, no
/// collection-behavior calls); closing it leaves the app running.
fn open_settings_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_focus();
        return;
    }
    match tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title("notchtap settings")
    .inner_size(480.0, 600.0)
    .build()
    {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => tracing::warn!("settings window failed to open: {e}"),
    }
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

#[cfg(target_os = "macos")]
fn dismiss_current<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    queue: &Arc<Mutex<SingleSlotQueue>>,
) {
    let mut q = queue.blocking_lock();
    q.dismiss_visible(Instant::now());
    if let Some(state) = q.slot_state_if_changed() {
        drop(q);
        emit_slot_state(app, state);
    }
}

#[cfg(target_os = "macos")]
fn open_current_story<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    queue: &Arc<Mutex<SingleSlotQueue>>,
) {
    let url = {
        let q = queue.blocking_lock();
        let Some(url) = q.current_link() else {
            tracing::debug!("open story ignored: no visible article link");
            return;
        };
        url.to_string()
    };

    let is_http = reqwest::Url::parse(&url)
        .map(|parsed| parsed.scheme() == "http" || parsed.scheme() == "https")
        .unwrap_or(false);
    if !is_http {
        tracing::debug!(%url, "open story ignored: link is not a valid http(s) url");
        return;
    }

    if let Err(error) = std::process::Command::new("open").arg(&url).spawn() {
        tracing::debug!(%error, %url, "open story command could not be spawned");
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::event::{
        Event, EventMeta, EventPayload, EventSignal, EventType, Priority, RotationSpec, SlotState,
        SourceKind,
    };

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
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: SourceKind::Manual,
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

    #[test]
    fn dismiss_current_promotes_next_waiting_item() {
        let app = tauri::test::mock_app();
        let mut inner = SingleSlotQueue::new(50);
        inner.enqueue(event(Priority::Medium)).unwrap();
        let next = event(Priority::Medium);
        let next_id = next.id;
        inner.enqueue(next).unwrap();
        let queue = Arc::new(Mutex::new(inner));

        dismiss_current(&app.handle().clone(), &queue);

        let q = queue.blocking_lock();
        match q.current_slot_state() {
            SlotState::Showing { id, .. } => assert_eq!(id, next_id),
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn dismiss_current_is_noop_when_slot_already_empty() {
        let app = tauri::test::mock_app();
        let mut inner = SingleSlotQueue::new(50);
        assert_eq!(inner.slot_state_if_changed(), Some(SlotState::Empty));
        let queue = Arc::new(Mutex::new(inner));

        dismiss_current(&app.handle().clone(), &queue);

        let mut q = queue.blocking_lock();
        assert_eq!(q.current_slot_state(), SlotState::Empty);
        assert!(q.slot_state_if_changed().is_none());
    }

    #[test]
    fn toggle_pause_updates_label_and_promotes_on_resume() {
        let app = tauri::test::mock_app();
        let pause_item =
            MenuItem::with_id(app.handle(), "pause", "Pause", true, None::<&str>).unwrap();
        let queue = Arc::new(Mutex::new(SingleSlotQueue::new(50)));

        toggle_pause(&app.handle().clone(), &queue, &pause_item);
        assert_eq!(pause_item.text().unwrap(), "Resume");
        {
            let mut q = queue.blocking_lock();
            assert!(q.is_paused());
            q.enqueue(event(Priority::Medium)).unwrap();
            assert_eq!(q.current_slot_state(), SlotState::Empty);
        }

        toggle_pause(&app.handle().clone(), &queue, &pause_item);
        assert_eq!(pause_item.text().unwrap(), "Pause");
        let q = queue.blocking_lock();
        assert!(!q.is_paused());
        assert!(matches!(q.current_slot_state(), SlotState::Showing { .. }));
        assert_eq!(q.total_waiting(), 0);
    }
}
