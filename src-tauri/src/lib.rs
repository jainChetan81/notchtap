mod about;
pub mod agents;
mod config;
mod crests;
mod engine;
mod click;
pub mod error;
pub mod event;
mod history;
mod hover;
mod http;
mod logging;
#[cfg(target_os = "macos")]
mod login_item;
mod net;
mod news_charge;
mod poller;
mod prefix;
mod presentation;
pub mod queue;
mod rss_poller;
mod settings;
// The single source of truth for the fifteen settings-window commands (see this module's own doc
// comment) — build.rs's AppManifest::commands allowlist.
mod settings_commands;
pub mod silence;
mod status;
mod tabs;
mod tray;

use std::sync::{Arc, Mutex as StdMutex, Once, OnceLock};

#[cfg(target_os = "macos")]
use tauri::menu::MenuItem;
use tauri::webview::PageLoadEvent;
#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;
use tauri::Manager;

use crate::config::Config;
use crate::crests::CrestCache;
use crate::engine::Engine;
use crate::history::HistoryStore;
use crate::queue::SingleSlotQueue;
use crate::settings::AppearanceChangedPayload;
use crate::tray::{build_tray, spawn_silence_task};
#[cfg(target_os = "macos")]
use crate::tray::{open_settings_window, toggle_pause};

#[cfg(target_os = "macos")]
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

// It fires normally even with `set_ignore_cursor_events(true)` (apply_overlay_native_config,
// below) permanently set (docs/design/hover-cursor-tracking.md §2).
#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(OverlayPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false
        }
        with: {
            tracking_area: {
                options: tauri_nspanel::TrackingAreaOptions::new()
                    .active_always()
                    .mouse_entered_and_exited()
                    .mouse_moved(),
                auto_resize: true
            }
        }
    })

    panel_event!(OverlayPanelEventHandler {})
}

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

#[cfg(target_os = "macos")]
const SKIP_SHORTCUT: (Option<Modifiers>, Code) = (
    Some(Modifiers::CONTROL.union(Modifiers::SHIFT)),
    Code::BracketRight,
);
#[cfg(target_os = "macos")]
const OPEN_SETTINGS_SHORTCUT: (Option<Modifiers>, Code) = (
    Some(Modifiers::CONTROL.union(Modifiers::SHIFT)),
    Code::Comma,
);

#[cfg(target_os = "macos")]
const FOCUS_SESSION_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyA);

// tracing-appender flushes through this guard; it must live as long as the process, so it's parked
// in a static rather than dropped at the end of run()'s setup.
static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

pub fn run() {
    match logging::init_logging() {
        Ok(guard) => {
            let _ = LOG_GUARD.set(guard);
        }
        Err(e) => eprintln!("notchtap: file logging unavailable: {e}"),
    }

    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("{e}");
            eprintln!("notchtap: {e}");
            // A blocking native dialog is the only way this failure is ever actually seen, so it
            // must be shown BEFORE the exit below, not logged only.
            show_boot_error_dialog(&format!(
                "notchtap couldn't start: config.toml is malformed ({e})"
            ));
            std::process::exit(1);
        }
    };

    // Boot-time parity with the settings window: config.toml is the other editing surface, so it
    // gets the same validation — but warn-and-continue.
    if let Err(violations) = crate::settings::validate(&config) {
        for v in &violations {
            tracing::warn!(violation = %v, "config.toml value out of range — running with it anyway");
        }
    }

    let (mode, inset, cutout) = presentation::detect_mode(&config);
    tracing::info!(?mode, inset, "presentation mode resolved");

    let mut initial_queue = SingleSlotQueue::new(config.max_queued_per_tier)
        .with_rotation_order(config.rotation_order.clone());
    if config.start_paused {
        initial_queue.pause();
        tracing::info!("start_paused: launching with promotion paused");
    }
    let start_paused = config.start_paused;
    let config_for_state = config.clone();
    let port = config.port;
    let default_ttl = config.default_ttl;
    let espn_enabled = config.espn_enabled;
    let espn_leagues = config.espn_leagues.clone();
    let espn_poll_secs = config.espn_poll_secs;
    let espn_priority = config.espn_priority;
    let espn_ttl_secs = config.espn_ttl_secs;
    let espn_live_card = config.espn_live_card;
    let espn_rich_events = config.espn_rich_events;
    // Crest PNGs are runtime-cached here, never committed to git.
    let crests = dirs::home_dir()
        .map(|h| CrestCache::new(Config::dir_from_home(&h).join("crests")))
        .unwrap_or_else(|| {
            tracing::warn!("could not determine home directory; crests will not be cached");
            CrestCache::new(std::path::PathBuf::from("crests"))
        });
    let rss_enabled = config.rss_enabled;
    let rss_feeds = config.rss_feeds.clone();
    let rss_topics = config.rss_topics.clone();
    let rss_poll_secs = config.rss_poll_secs;
    let rss_priority = config.rss_priority;
    let rss_ttl_secs = config.rss_ttl_secs;
    let rss_max_per_poll = config.rss_max_per_poll;
    let manual_default_priority = config.manual_default_priority;
    let agent_priority = config.agent_priority;
    let agent_ttl_secs = config.agent_ttl_secs;
    let agents_config = config.agents.clone();
    let agent_notification_policy = agents::notification::NotificationPolicy {
        informational_notifications: agents_config.informational_notifications,
        completion_notifications: agents_config.completion_notifications,
        permission_priority: agents_config.permission_priority,
        input_priority: agents_config.input_priority,
        failure_priority: agents_config.failure_priority,
        completion_priority: agents_config.completion_priority,
    };
    let agent_runtimes = agents_config.runtimes;
    let agent_enabled = agents_config.enabled;
    let agent_board_show_working = agents_config.board_show_working;
    let agent_stale_after = std::time::Duration::from_secs(agents_config.stale_after_secs);
    let agent_terminal_retention =
        std::time::Duration::from_secs(agents_config.terminal_retention_secs);
    let agent_stale_retention = std::time::Duration::from_secs(agents_config.stale_retention_secs);
    let history_enabled = config.history_enabled;
    // the `[silence]` block feeds `SilenceController::new` at boot (below, in `setup`) —
    // session-only mute/skip state is never read from config, only the daily schedule.
    let silence_schedule_enabled = config.silence.enabled;
    let silence_window = config.silence.window;

    let server_once = Arc::new(Once::new());

    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    builder
        .invoke_handler(tauri::generate_handler![
            settings::clear_history,
            settings::clear_queue,
            settings::get_config,
            settings::get_default_config,
            settings::get_history,
            settings::get_queue,
            settings::get_recent_log_lines,
            settings::save_config_and_relaunch,
            settings::search_news_now,
            settings::send_test_notification,
            settings::set_appearance,
            settings::skip_current,
            settings::get_about_info,
            settings::get_agent_health,
            settings::send_agent_test_event,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(ActivationPolicy::Accessory);
            app.manage(std::time::Instant::now());
            app.manage(StdMutex::new(config_for_state));
            app.manage(StdMutex::new(rss_poller::SeenStore::default()));
            app.manage(std::sync::atomic::AtomicBool::new(false));
            let history = if history_enabled {
                match dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))
                    .and_then(|h| Ok(HistoryStore::new(Config::dir_from_home(&h))?))
                {
                    Ok(store) => Some(Arc::new(store)),
                    Err(e) => {
                        tracing::warn!(error = %e, "history disabled: could not open store");
                        None
                    }
                }
            } else {
                None
            };
            let tab_wire = std::sync::Arc::new(tabs::TabWire::new(rss_max_per_poll));
            app.manage(tab_wire.clone());
            let engine = Engine::new(
                initial_queue,
                app.handle().clone(),
                espn_enabled,
                rss_enabled,
                history,
                tab_wire.clone(),
            );
            app.manage(engine.clone());

            let silence_controller = Arc::new(StdMutex::new(silence::SilenceController::new(
                silence_schedule_enabled,
                silence_window,
            )));
            app.manage(silence_controller.clone());

            let agent_registry = agents::registry::AgentRegistryHandle::new(
                agents::registry::AgentRegistry::new(
                    agent_stale_after,
                    agent_terminal_retention,
                    agent_stale_retention,
                ),
            );
            app.manage(agent_registry.clone());

            // the shared Adapter Health tracker — managed exactly like `agent_registry` above so
            // `server_once`'s `http::AppState` (below), `agent_board`'s own publish path.
            let agent_health = std::sync::Arc::new(agents::health::HealthTracker::new());
            app.manage(agent_health.clone());

            let agent_board = agents::board::AgentBoardPublisher::new(
                app.handle().clone(),
                agent_registry,
                agent_health.clone(),
                agent_runtimes,
                agent_board_show_working,
                tab_wire.clone(),
            );
            app.manage(agent_board.clone());
            agent_board.spawn_tick(agents::board::DEFAULT_TICK_INTERVAL);

            let window = app
                .get_webview_window("main")
                .expect("main window missing from tauri.conf.json");
            window.set_always_on_top(true)?;

            #[cfg(target_os = "macos")]
            let was_hovered = Arc::new(StdMutex::new(false));

            #[cfg(target_os = "macos")]
            let board_frame = Arc::new(StdMutex::new(BoardFrameState::default()));

            // a plain NSWindow is never composited into another app's fullscreen Space, regardless
            // of level or collection behavior.
            #[cfg(target_os = "macos")]
            {
                use tauri_nspanel::WebviewWindowExt as _;
                let panel = window
                    .to_panel::<OverlayPanel>()
                    .map_err(|e| format!("nspanel conversion failed: {e:?}"))?;
                panel.set_style_mask(objc2_app_kit::NSWindowStyleMask::NonactivatingPanel);

                let hover_handler = OverlayPanelEventHandler::new();
                let hover_cutout_width = cutout.map(|c| c.width).unwrap_or(0.0);
                let hover_cutout_height = inset;

                {
                    let engine = engine.clone();
                    let app_handle = app.handle().clone();
                    let was_hovered = was_hovered.clone();
                    let agent_board = agent_board.clone();
                    let board_frame = board_frame.clone();
                    let tab_wire = tab_wire.clone();
                    let window = window.clone();
                    hover_handler.on_mouse_entered(move |event| {
                        let loc = event.locationInWindow();
                        let hover_latched = *was_hovered.lock().unwrap_or_else(|e| e.into_inner());
                        // the REAL currently-applied window height — `hover::WINDOW_HEIGHT` at
                        // rest, or the taller applied board-expand frame — never the constant.
                        let real_window_height =
                            board_frame.lock().unwrap_or_else(|e| e.into_inner()).height;
                        let hovered = hover_point_is_over_card(
                            &engine,
                            &app_handle,
                            mode,
                            hover_cutout_width,
                            hover_cutout_height,
                            hover_latched,
                            agent_board.last_session_count(),
                            real_window_height,
                            loc.x,
                            loc.y,
                        );
                        emit_hover_changed_if_transitioned(
                            &engine,
                            &app_handle,
                            &was_hovered,
                            hovered,
                            &window,
                            mode,
                            cutout,
                            &agent_board,
                            &board_frame,
                            &tab_wire,
                        );
                    });
                }
                {
                    let engine = engine.clone();
                    let app_handle = app.handle().clone();
                    let was_hovered = was_hovered.clone();
                    let agent_board = agent_board.clone();
                    let board_frame = board_frame.clone();
                    let tab_wire = tab_wire.clone();
                    let window = window.clone();
                    hover_handler.on_mouse_moved(move |event| {
                        let loc = event.locationInWindow();
                        let hover_latched = *was_hovered.lock().unwrap_or_else(|e| e.into_inner());
                        let real_window_height =
                            board_frame.lock().unwrap_or_else(|e| e.into_inner()).height;
                        let hovered = hover_point_is_over_card(
                            &engine,
                            &app_handle,
                            mode,
                            hover_cutout_width,
                            hover_cutout_height,
                            hover_latched,
                            agent_board.last_session_count(),
                            real_window_height,
                            loc.x,
                            loc.y,
                        );
                        emit_hover_changed_if_transitioned(
                            &engine,
                            &app_handle,
                            &was_hovered,
                            hovered,
                            &window,
                            mode,
                            cutout,
                            &agent_board,
                            &board_frame,
                            &tab_wire,
                        );
                    });
                }
                {
                    let engine = engine.clone();
                    let app_handle = app.handle().clone();
                    let was_hovered = was_hovered.clone();
                    let agent_board = agent_board.clone();
                    let board_frame = board_frame.clone();
                    let tab_wire = tab_wire.clone();
                    let window = window.clone();
                    // Leaving the window's tracking area is never "still hovered" regardless of
                    // where the cursor lands next — no rect comparison needed.
                    hover_handler.on_mouse_exited(move |_event| {
                        emit_hover_changed_if_transitioned(
                            &engine,
                            &app_handle,
                            &was_hovered,
                            false,
                            &window,
                            mode,
                            cutout,
                            &agent_board,
                            &board_frame,
                            &tab_wire,
                        );
                    });
                }

                panel.set_event_handler(Some(hover_handler.as_ref()));

                {
                    use tauri::Listener;
                    let was_hovered = was_hovered.clone();
                    let last_visible_id: Arc<StdMutex<Option<String>>> =
                        Arc::new(StdMutex::new(None));
                    let board_frame = board_frame.clone();
                    let window = window.clone();
                    app.handle()
                        .listen(crate::event::SLOT_STATE_EVENT, move |event| {
                            let new_id = visible_id_from_slot_state_payload(event.payload());
                            let is_real_notification = new_id.is_some();
                            let mut last =
                                last_visible_id.lock().unwrap_or_else(|e| e.into_inner());
                            if *last != new_id {
                                *last = new_id;
                                let was = std::mem::replace(
                                    &mut *was_hovered.lock().unwrap_or_else(|e| e.into_inner()),
                                    false,
                                );
                                if was {
                                    let _ = window.set_ignore_cursor_events(true);
                                }
                                if is_real_notification {
                                    collapse_board_if_expanded(&window, mode, cutout, &board_frame);
                                }
                            }
                        });
                }
            }

            // the icon-strip click monitor — an NSEvent LOCAL monitor (click.rs's module doc
            // records why nothing else satisfies the receive-only overlay).
            #[cfg(target_os = "macos")]
            {
                let monitor_cutout_width = cutout.map(|c| c.width).unwrap_or(0.0);
                let monitor_cutout_height = inset;
                let window_number = {
                    use objc2_app_kit::NSWindow;
                    let ns_window_ptr = window.ns_window()? as *mut NSWindow;
                    let ns_window: &NSWindow = unsafe { &*ns_window_ptr };
                    ns_window.windowNumber()
                };
                click::install_click_monitor(click::ClickMonitorParams {
                    app: app.handle().clone(),
                    tab_wire: tab_wire.clone(),
                    was_hovered: was_hovered.clone(),
                    window_number,
                    mode,
                    cutout_width: monitor_cutout_width,
                    cutout_height: monitor_cutout_height,
                    board_frame: board_frame.clone(),
                });
            }

            #[cfg(target_os = "macos")]
            apply_overlay_native_config(&window)?;

            position_window(&window, mode, cutout)?;
            let (pause_item, silenced_indicator_item) = build_tray(
                app.handle(),
                engine.clone(),
                start_paused,
                silence_controller.clone(),
            )?;

            #[cfg(target_os = "macos")]
            {
                // An unparseable key logs and falls back to Space — fail-open, matching the
                // validator's own permissive grammar (settings::is_valid_prefix_shortcut).
                let prefix_sc = prefix_shortcut_from_config(&config.prefix_shortcut);
                let prefix_sc_for_handler = prefix_sc;
                let engine_for_handler = engine.clone();
                let pause_item_for_handler = pause_item.clone();
                let was_hovered_for_handler = was_hovered.clone();
                let tab_wire_for_handler = tab_wire.clone();
                let agent_board_for_handler = agent_board.clone();
                let board_frame_for_handler = board_frame.clone();
                let window_for_handler = window.clone();
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
                                    toggle_manual_expand(&engine_for_handler);
                                } else if *shortcut
                                    == Shortcut::new(OPEN_STORY_SHORTCUT.0, OPEN_STORY_SHORTCUT.1)
                                {
                                    open_current_story(&engine_for_handler);
                                } else if *shortcut
                                    == Shortcut::new(DISMISS_SHORTCUT.0, DISMISS_SHORTCUT.1)
                                {
                                    dismiss_current(&engine_for_handler);
                                    // the dismiss hotkey replaces the visible card with no mouse
                                    // event firing.
                                    emit_hover_changed_if_transitioned(
                                        &engine_for_handler,
                                        app,
                                        &was_hovered_for_handler,
                                        false,
                                        &window_for_handler,
                                        mode,
                                        cutout,
                                        &agent_board_for_handler,
                                        &board_frame_for_handler,
                                        &tab_wire_for_handler,
                                    );
                                } else if *shortcut
                                    == Shortcut::new(
                                        PAUSE_TOGGLE_SHORTCUT.0,
                                        PAUSE_TOGGLE_SHORTCUT.1,
                                    )
                                {
                                    toggle_pause(&engine_for_handler, &pause_item_for_handler);
                                } else if *shortcut
                                    == Shortcut::new(SKIP_SHORTCUT.0, SKIP_SHORTCUT.1)
                                {
                                    skip_current(&engine_for_handler);
                                    emit_hover_changed_if_transitioned(
                                        &engine_for_handler,
                                        app,
                                        &was_hovered_for_handler,
                                        false,
                                        &window_for_handler,
                                        mode,
                                        cutout,
                                        &agent_board_for_handler,
                                        &board_frame_for_handler,
                                        &tab_wire_for_handler,
                                    );
                                } else if *shortcut
                                    == Shortcut::new(
                                        OPEN_SETTINGS_SHORTCUT.0,
                                        OPEN_SETTINGS_SHORTCUT.1,
                                    )
                                {
                                    open_settings_window(app);
                                } else if *shortcut == prefix_sc_for_handler {
                                    handle_prefix_fire(
                                        app,
                                        &tab_wire_for_handler,
                                    );
                                } else if let Some(key) =
                                    prefix_followup_key_for(shortcut)
                                {
                                    handle_prefix_followup(
                                        app,
                                        key,
                                        &tab_wire_for_handler,
                                        &engine_for_handler,
                                        &pause_item_for_handler,
                                    );
                                } else if *shortcut
                                    == Shortcut::new(
                                        FOCUS_SESSION_SHORTCUT.0,
                                        FOCUS_SESSION_SHORTCUT.1,
                                    )
                                {
                                    // Rust-only, no overlay involvement — the overlay stays
                                    // receive-only.
                                    let registry = app
                                        .state::<agents::registry::AgentRegistryHandle>()
                                        .inner()
                                        .clone();
                                    tauri::async_runtime::spawn(async move {
                                        let now = std::time::Instant::now();
                                        let states = registry.ordered_states(now).await;
                                        agents::focus::focus_highest_ranked(&states);
                                    });
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
                app.global_shortcut()
                    .register(Shortcut::new(SKIP_SHORTCUT.0, SKIP_SHORTCUT.1))?;
                app.global_shortcut().register(Shortcut::new(
                    OPEN_SETTINGS_SHORTCUT.0,
                    OPEN_SETTINGS_SHORTCUT.1,
                ))?;
                app.global_shortcut().register(Shortcut::new(
                    FOCUS_SESSION_SHORTCUT.0,
                    FOCUS_SESSION_SHORTCUT.1,
                ))?;
                if let Err(e) = app
                    .global_shortcut()
                    .register(prefix_shortcut_from_config(&config.prefix_shortcut))
                {
                    tracing::warn!(
                        "prefix shortcut {:?} failed to register: {e}",
                        config.prefix_shortcut
                    );
                }
            }

            #[cfg(target_os = "macos")]
            login_item::register();
            // the rotation loop lives inside the Engine — it is the consumer of the wake, so the
            // wake never escapes engine.rs.
            engine.spawn_rotation();

            spawn_silence_task(
                engine.clone(),
                silence_controller.clone(),
                silenced_indicator_item,
            );

            #[cfg(target_os = "macos")]
            {
                const SESSION_AUTO_ADVANCE_INTERVAL: std::time::Duration =
                    std::time::Duration::from_secs(6);
                let auto_advance_wire = tab_wire.clone();
                let auto_advance_app = app.handle().clone();
                let auto_advance_engine = engine.clone();
                let auto_advance_hovered = was_hovered.clone();
                tauri::async_runtime::spawn(async move {
                    use std::sync::atomic::Ordering;
                    loop {
                        tokio::select! {
                            _ = tokio::time::sleep(SESSION_AUTO_ADVANCE_INTERVAL) => {}
                            _ = auto_advance_wire.session_advanced.notified() => {
                                continue;
                            }
                        }
                        let tab_selected = {
                            let sel = auto_advance_wire
                                .tabs
                                .selection
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            sel.selected()
                        };
                        let session_count = auto_advance_wire.agent_sessions.load(Ordering::Relaxed);
                        let hovered = *auto_advance_hovered.lock().unwrap_or_else(|e| e.into_inner());
                        // Checking `is_paused()` is a pure read — it never mutates the queue — so
                        // this must go through `Engine::read`, NOT `apply`/`apply_blocking`.
                        let paused = auto_advance_engine.read(|q| q.is_paused()).await;
                        if should_auto_advance_session(tab_selected, session_count, hovered, paused) {
                            let current = auto_advance_wire.viewed_session.load(Ordering::Relaxed) as isize;
                            let next = (current + 1).rem_euclid(session_count as isize) as usize;
                            auto_advance_wire.viewed_session.store(next, Ordering::Relaxed);
                            use tauri::Emitter;
                            if let Err(e) = auto_advance_app.emit(
                                "agent-viewed-session-changed",
                                serde_json::json!({ "index": next }),
                            ) {
                                tracing::error!(
                                    "failed to emit agent-viewed-session-changed (auto-advance): {e}"
                                );
                            }
                        }
                    }
                });
            }

            // espn poller — config-gated: `espn_enabled = false` means it never spawns.
            if espn_enabled {
                poller::spawn_espn_poller(
                    engine.clone(),
                    espn_leagues,
                    espn_poll_secs,
                    espn_ttl_secs,
                    espn_priority,
                    espn_live_card,
                    espn_rich_events,
                    crests.clone(),
                );
            }
            if rss_enabled {
                rss_poller::spawn_rss_poller(
                    engine.clone(),
                    app.handle().clone(),
                    rss_feeds,
                    rss_topics,
                    rss_poll_secs,
                    rss_ttl_secs,
                    rss_max_per_poll,
                    rss_priority,
                    tab_wire.clone(),
                );
            }

            Ok(())
        })
        .on_page_load(move |webview, payload| {
            if payload.event() == PageLoadEvent::Finished && webview.label() == "main" {
                let app_handle = webview.app_handle().clone();
                let engine = app_handle.state::<Engine>().inner().clone();

                {
                    let current_state = engine.current_slot_state_blocking();
                    let state_json =
                        serde_json::to_string(&current_state).unwrap_or_else(|_| "null".into());
                    let safe_json = escape_for_eval_splice(&state_json);
                    let _ = webview.eval(format!("window.__NOTCHTAP_SLOT_STATE__ = {safe_json};"));
                    crate::event::emit_slot_state(&app_handle, current_state);
                }

                {
                    let current_status = engine.status_snapshot_blocking();
                    let status_json =
                        serde_json::to_string(&current_status).unwrap_or_else(|_| "null".into());
                    let safe_json = escape_for_eval_splice(&status_json);
                    let _ =
                        webview.eval(format!("window.__NOTCHTAP_STATUS_STATE__ = {safe_json};"));
                    crate::status::emit_status_state(&app_handle, current_status);
                }

                // Double-shield the initial appearance values the same way as slot state above: a
                // global for the React mount race, plus an emit for listeners already registered.
                {
                    use tauri::Emitter;
                    let config = app_handle.state::<StdMutex<Config>>().lock().unwrap().clone();
                    let payload = AppearanceChangedPayload::from_config(&config);
                    let payload_json = escape_for_eval_splice(
                        &serde_json::to_string(&payload).unwrap_or_else(|_| "null".into()),
                    );
                    let _ =
                        webview.eval(format!("window.__NOTCHTAP_APPEARANCE__ = {payload_json};"));
                    let _ = webview.emit("appearance-changed", &payload);
                }

                {
                    let mode_str = match mode {
                        presentation::Mode::Notch => "notch",
                        presentation::Mode::Hud => "hud",
                    };
                    let width_json = cutout_width_js_value(cutout);
                    let height_json = cutout_height_js_value(inset);
                    let _ = webview.eval(format!(
                        "window.__NOTCHTAP_MODE__ = \"{mode_str}\"; window.__NOTCHTAP_CUTOUT_WIDTH__ = {width_json}; window.__NOTCHTAP_CUTOUT_HEIGHT__ = {height_json};"
                    ));
                }

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
                        engine: app_handle.state::<Engine>().inner().clone(),
                        default_ttl,
                        manual_default_priority,
                        agent_priority,
                        agent_ttl_secs,
                        agent_notification_policy,
                        agent_runtimes,
                        agent_enabled,
                        agent_registry: app_handle
                            .state::<agents::registry::AgentRegistryHandle>()
                            .inner()
                            .clone(),
                        agent_board: app_handle
                            .state::<agents::board::AgentBoardPublisher>()
                            .inner()
                            .clone(),
                        agent_health: app_handle
                            .state::<std::sync::Arc<agents::health::HealthTracker>>()
                            .inner()
                            .clone(),
                    };
                    tauri::async_runtime::spawn(async move {
                        let listener = match http::bind_listener(port).await {
                            Ok(l) => l,
                            Err(e) => {
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
        .build(tauri::generate_context!())
        .expect("error while running notchtap")
        .run(|app_handle, event| {
            #[cfg(target_os = "macos")]
            if matches!(event, tauri::RunEvent::Exit) {
                if let Some(tab_wire) = app_handle.try_state::<Arc<tabs::TabWire>>() {
                    force_release_prefix_followups(app_handle, &tab_wire);
                }
            }
        });
}

#[cfg(target_os = "macos")]
fn show_boot_error_dialog(message: &str) {
    let script = format!(
        "display dialog \"{}\" with title \"notchtap\" buttons {{\"Quit\"}} default button \"Quit\" with icon stop",
        escape_for_osascript(message)
    );
    let _ = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(&script)
        .status();
}

#[cfg(not(target_os = "macos"))]
fn show_boot_error_dialog(_message: &str) {}

#[cfg(target_os = "macos")]
fn escape_for_osascript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_for_eval_splice(json: &str) -> String {
    json.replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
        .replace('<', "\\u003c")
}

// The window must overlap the menu bar (flush to y=0), survive Spaces switches, and stay visible
// over fullscreen apps.
#[cfg(target_os = "macos")]
fn apply_overlay_native_config(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2_app_kit::{NSStatusWindowLevel, NSWindow, NSWindowCollectionBehavior};
    window.set_ignore_cursor_events(true)?;
    window.set_visible_on_all_workspaces(true)?;
    let ns_window_ptr = window.ns_window()? as *mut NSWindow;
    let ns_window: &NSWindow = unsafe { &*ns_window_ptr };
    // set the EXACT behavior, never OR with the current bits: tao puts FullScreenNone on
    // non-resizable windows.
    let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::Stationary
        | NSWindowCollectionBehavior::IgnoresCycle;
    ns_window.setCollectionBehavior(behavior);
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

fn cutout_width_js_value(cutout: Option<presentation::CutoutGeometry>) -> String {
    match cutout {
        Some(c) => format!("{}", c.width),
        None => "null".into(),
    }
}

fn cutout_height_js_value(inset: f64) -> String {
    if inset > 0.0 {
        format!("{inset}")
    } else {
        "null".into()
    }
}

// Lock discipline: each of `engine.read_blocking`/the config lock acquires, reads, and drops
// before the next opens — never nested.
#[allow(clippy::too_many_arguments)]
#[cfg(target_os = "macos")]
fn hover_point_is_over_card(
    engine: &Engine,
    app_handle: &tauri::AppHandle,
    mode: presentation::Mode,
    cutout_width: f64,
    cutout_height: f64,
    hover_latched: bool,
    board_session_count: usize,
    real_window_height: f64,
    point_x: f64,
    point_y: f64,
) -> bool {
    use crate::event::SlotState;

    let (visible, expanded) = engine.read_blocking(|q| match q.current_slot_state() {
        SlotState::Showing { expanded, .. } => (true, expanded),
        SlotState::Empty => (false, false),
    });
    // Poison-tolerant: a panic elsewhere must not turn every later mouse event into a panic inside
    // an objc callback (this runs on the AppKit main thread, from the tracking area's handlers).
    let scale = app_handle
        .state::<StdMutex<Config>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .appearance
        .card_scale;
    let rect = if !visible && board_session_count > 0 {
        hover::board_rect(
            mode,
            cutout_width,
            cutout_height,
            scale,
            board_session_count,
            real_window_height,
        )
    } else {
        hover::active_card_rect(
            mode,
            cutout_width,
            cutout_height,
            scale,
            visible,
            expanded,
            hover_latched,
            hover_latched,
        )
    };
    hover::point_in_rect(&rect, point_x, point_y)
}

#[allow(clippy::too_many_arguments)]
#[cfg(target_os = "macos")]
fn emit_hover_changed_if_transitioned(
    engine: &Engine,
    app_handle: &tauri::AppHandle,
    was_hovered: &StdMutex<bool>,
    hovered: bool,
    window: &tauri::WebviewWindow,
    mode: presentation::Mode,
    cutout: Option<presentation::CutoutGeometry>,
    agent_board: &agents::board::AgentBoardPublisher,
    board_frame: &Arc<StdMutex<BoardFrameState>>,
    tab_wire: &Arc<tabs::TabWire>,
) {
    use tauri::Emitter;

    {
        let mut last = was_hovered.lock().unwrap_or_else(|e| e.into_inner());
        if *last == hovered {
            return;
        }
        *last = hovered;
    }
    if let Some(webview) = app_handle.get_webview_window("main") {
        let _ = webview.emit("hover-changed", &serde_json::json!({ "hovered": hovered }));
    }
    engine.apply_blocking(|q, now| {
        if hovered {
            q.hover_enter(now);
        } else {
            q.hover_exit(now);
        }
    });

    // The restore is split: click-through comes back in this tick, the frame shrink is deferred by
    // a grace period so the webview's collapse spring isn't clipped.
    if hovered {
        try_expand_board_for_hover(engine, window, agent_board, board_frame);
    } else {
        collapse_board_if_expanded(window, mode, cutout, board_frame);
    }

    if hovered {
        if !tab_wire
            .slot_occupied
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            if let Err(e) = window.set_ignore_cursor_events(false) {
                tracing::warn!("icon strip: set_ignore_cursor_events(false) failed: {e}");
            }
        }
    } else {
        let board_expanded = board_frame
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .expanded;
        if !board_expanded {
            if let Err(e) = window.set_ignore_cursor_events(true) {
                tracing::warn!("icon strip: set_ignore_cursor_events(true) failed: {e}");
            }
        }
    }
}

/// Every field is read/written under ONE lock so a pending shrink timer can never observe a
/// half-updated pair.
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BoardFrameState {
    /// Stays `true` for the whole grace period after a hover-exit — the frame really is still the
    /// big one until the timer shrinks it.
    expanded: bool,
    generation: u64,
    pub(crate) height: f64,
}

#[cfg(target_os = "macos")]
impl Default for BoardFrameState {
    fn default() -> Self {
        BoardFrameState {
            expanded: false,
            generation: 0,
            height: hover::WINDOW_HEIGHT,
        }
    }
}

#[cfg(target_os = "macos")]
const BOARD_COLLAPSE_GRACE_MS: u64 = 450;

#[cfg(target_os = "macos")]
fn board_shrink_should_run(state: BoardFrameState, armed_generation: u64) -> bool {
    state.expanded && state.generation == armed_generation
}

/// On a hover ENTRY, expand the Board's window frame + open pointer delivery.
#[cfg(target_os = "macos")]
fn try_expand_board_for_hover(
    engine: &Engine,
    window: &tauri::WebviewWindow,
    agent_board: &agents::board::AgentBoardPublisher,
    board_frame: &StdMutex<BoardFrameState>,
) {
    use crate::event::SlotState;

    let visible =
        engine.read_blocking(|q| matches!(q.current_slot_state(), SlotState::Showing { .. }));
    let session_count = agent_board.last_session_count();
    if visible || session_count == 0 {
        return;
    }
    let Ok(Some(monitor)) = window.current_monitor() else {
        tracing::warn!("board hover-expand: no current monitor; skipping");
        return;
    };
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let screen_size = monitor.size().to_logical::<f64>(scale_factor);
    let frame =
        agents::expand::expanded_board_frame(screen_size.width, screen_size.height, session_count);
    // Order matters: grow the frame FIRST, then open pointer delivery — never the reverse.
    if let Err(e) = window.set_size(tauri::LogicalSize::new(frame.width, frame.height)) {
        tracing::warn!("board hover-expand: set_size failed: {e}");
        return;
    }
    board_frame.lock().unwrap_or_else(|e| e.into_inner()).height = frame.height;
    if let Err(e) = window.set_position(tauri::LogicalPosition::new(frame.x, frame.y)) {
        tracing::warn!("board hover-expand: set_position failed: {e}");
    }
    if let Err(e) = window.set_ignore_cursor_events(false) {
        tracing::warn!("board hover-expand: set_ignore_cursor_events(false) failed: {e}");
        return;
    }
    let mut state = board_frame.lock().unwrap_or_else(|e| e.into_inner());
    state.expanded = true;
    state.generation = state.generation.wrapping_add(1);
}

/// Idempotent: a hover-exit over a card that never expanded anything (`expanded == false` already)
/// does nothing, so this is safe to call from every `hovered == false` path unconditionally.
#[cfg(target_os = "macos")]
fn collapse_board_if_expanded(
    window: &tauri::WebviewWindow,
    mode: presentation::Mode,
    cutout: Option<presentation::CutoutGeometry>,
    board_frame: &Arc<StdMutex<BoardFrameState>>,
) {
    let armed_generation = {
        let mut state = board_frame.lock().unwrap_or_else(|e| e.into_inner());
        if !state.expanded {
            return;
        }
        state.generation = state.generation.wrapping_add(1);
        state.generation
    };

    // IMMEDIATE, never deferred — see the doc comment.
    if let Err(e) = window.set_ignore_cursor_events(true) {
        tracing::warn!("board hover-collapse: set_ignore_cursor_events(true) failed: {e}");
    }

    let window = window.clone();
    let board_frame = board_frame.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(BOARD_COLLAPSE_GRACE_MS)).await;
        let shrink_window = window.clone();
        let _ = window.run_on_main_thread(move || {
            let mut state = board_frame.lock().unwrap_or_else(|e| e.into_inner());
            if !board_shrink_should_run(*state, armed_generation) {
                return;
            }
            let shrank = match shrink_window.set_size(tauri::LogicalSize::new(
                hover::WINDOW_WIDTH,
                hover::WINDOW_HEIGHT,
            )) {
                Ok(()) => true,
                Err(e) => {
                    tracing::warn!("board hover-collapse: set_size failed: {e}");
                    false
                }
            };
            if let Err(e) = position_window(&shrink_window, mode, cutout) {
                tracing::warn!("board hover-collapse: position_window failed: {e}");
            }
            state.expanded = false;
            // Only claim the resting height if the shrink actually landed: on a failed `set_size`
            // the window is still tall, and `state.height` must not claim otherwise.
            if shrank {
                state.height = hover::WINDOW_HEIGHT;
            }
        });
    });
}

#[cfg(target_os = "macos")]
fn visible_id_from_slot_state_payload(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()?
        .get("id")
        .and_then(|id| id.as_str())
        .map(str::to_string)
}

fn position_window(
    window: &tauri::WebviewWindow,
    mode: presentation::Mode,
    cutout: Option<presentation::CutoutGeometry>,
) -> tauri::Result<()> {
    if let (presentation::Mode::Notch, Some(cutout)) = (mode, cutout) {
        let scale_factor = window.scale_factor()?;
        let win_width = window.outer_size()?.to_logical::<f64>(scale_factor).width;
        let x = cutout.center_x() - (win_width / 2.0);

        // coordinate-space invariant: NSScreen reports points (= logical px, global origin);
        // tauri's LogicalPosition shares the x-axis on the primary display.
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

// Comfortably past `PREFIX_ARM_WINDOW` (2s) so it never races a legitimate window, short enough
// that a stuck bare `Enter` is measured in seconds rather than "until the app restarts".
#[cfg(target_os = "macos")]
const PREFIX_WATCHDOG_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[cfg(target_os = "macos")]
const PREFIX_FOLLOWUPS: [(Code, prefix::PrefixKey); 9] = [
    (Code::Digit1, prefix::PrefixKey::Digit(1)),
    (Code::Digit2, prefix::PrefixKey::Digit(2)),
    (Code::Digit3, prefix::PrefixKey::Digit(3)),
    (Code::BracketLeft, prefix::PrefixKey::BracketLeft),
    (Code::BracketRight, prefix::PrefixKey::BracketRight),
    (Code::Enter, prefix::PrefixKey::ExpandToggle),
    (Code::KeyO, prefix::PrefixKey::ExpandToggle),
    (Code::KeyP, prefix::PrefixKey::Pause),
    (Code::Escape, prefix::PrefixKey::Disarm),
];

/// Unresolvable keys warn and fall back to Space rather than failing boot (fail-open, same posture
/// as every optional surface here).
#[cfg(target_os = "macos")]
fn prefix_shortcut_from_config(value: &str) -> Shortcut {
    use std::str::FromStr;
    let key = value.strip_prefix("\u{2303}\u{21e7}").unwrap_or(value);
    let code = Code::from_str(key)
        .ok()
        .or_else(|| {
            let mut chars = key.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphabetic() => {
                    Code::from_str(&format!("Key{}", c.to_ascii_uppercase())).ok()
                }
                (Some(c), None) if c.is_ascii_digit() => Code::from_str(&format!("Digit{c}")).ok(),
                _ => None,
            }
        })
        .unwrap_or_else(|| {
            tracing::warn!(?value, "unresolvable prefix key — falling back to Space");
            Code::Space
        });
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), code)
}

#[cfg(target_os = "macos")]
fn prefix_followup_key_for(shortcut: &Shortcut) -> Option<prefix::PrefixKey> {
    PREFIX_FOLLOWUPS
        .iter()
        .find(|(code, _)| *shortcut == Shortcut::new(None, *code))
        .map(|(_, key)| *key)
}

#[cfg(target_os = "macos")]
fn set_prefix_followups_registered<R: tauri::Runtime>(app: &tauri::AppHandle<R>, on: bool) -> bool {
    let mut all_ok = true;
    for (code, _) in PREFIX_FOLLOWUPS {
        let sc = Shortcut::new(None, code);
        let result = if on {
            app.global_shortcut().register(sc)
        } else {
            app.global_shortcut().unregister(sc)
        };
        if let Err(e) = result {
            all_ok = false;
            if on {
                tracing::warn!(?code, "prefix follow-up grab failed: {e}");
            } else {
                tracing::error!(
                    ?code,
                    "prefix follow-up RELEASE failed — this key may stay grabbed system-wide: {e}"
                );
            }
        }
    }
    all_ok
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatchdogVerdict {
    Done,
    Reschedule(std::time::Duration),
    Release,
}

fn watchdog_verdict(
    followups_registered: bool,
    last_arm_at: Option<std::time::Instant>,
    now: std::time::Instant,
    timeout: std::time::Duration,
) -> WatchdogVerdict {
    if !followups_registered {
        return WatchdogVerdict::Done;
    }
    let Some(armed_at) = last_arm_at else {
        return WatchdogVerdict::Release;
    };
    match timeout.checked_sub(now.saturating_duration_since(armed_at)) {
        Some(remaining) if !remaining.is_zero() => WatchdogVerdict::Reschedule(remaining),
        _ => WatchdogVerdict::Release,
    }
}

#[cfg(target_os = "macos")]
fn force_release_prefix_followups<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    tab_wire: &Arc<tabs::TabWire>,
) {
    use std::sync::atomic::Ordering;
    if !tab_wire.followups_registered.swap(false, Ordering::SeqCst) {
        return;
    }
    tracing::warn!("prefix watchdog: force-releasing follow-up grabs");
    let all_ok = set_prefix_followups_registered(app, false);
    if !all_ok {
        // The invariant every caller honours (see `handle_prefix_fire` /
        // `handle_prefix_followup`): a failed RELEASE keeps the flag true so the watchdog retries.
        tab_wire.followups_registered.store(true, Ordering::SeqCst);
    }
    *tab_wire.prefix.lock().unwrap_or_else(|e| e.into_inner()) = prefix::PrefixState::Disarmed;
}

#[cfg(target_os = "macos")]
fn handle_prefix_fire<R: tauri::Runtime>(app: &tauri::AppHandle<R>, tab_wire: &Arc<tabs::TabWire>) {
    use std::sync::atomic::Ordering;
    let now = std::time::Instant::now();
    let armed = {
        let mut st = tab_wire.prefix.lock().unwrap_or_else(|e| e.into_inner());
        st.on_prefix(now);
        st.is_armed(now)
    };
    let generation = tab_wire.prefix_generation.fetch_add(1, Ordering::SeqCst) + 1;
    if armed {
        *tab_wire
            .last_arm_at
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(now);
    }
    let all_ok = set_prefix_followups_registered(app, armed);
    tab_wire
        .followups_registered
        .store(armed || !all_ok, Ordering::SeqCst);
    if armed {
        let timer_app = app.clone();
        let wire = tab_wire.clone();
        tauri::async_runtime::spawn(async move {
            let app = timer_app;
            tokio::time::sleep(prefix::PREFIX_ARM_WINDOW).await;
            if wire.prefix_generation.load(Ordering::SeqCst) == generation {
                {
                    let mut st = wire.prefix.lock().unwrap_or_else(|e| e.into_inner());
                    *st = prefix::PrefixState::Disarmed;
                }
                if set_prefix_followups_registered(&app, false) {
                    wire.followups_registered.store(false, Ordering::SeqCst);
                }
            }
        });
        // Generation-blind BY DESIGN (a generation gate would blind it to the "follow-up consumed,
        // but its release failed" case) but DEADLINE-aware.
        let watchdog_app = app.clone();
        let watchdog_wire = tab_wire.clone();
        tauri::async_runtime::spawn(async move {
            let mut sleep_for = PREFIX_WATCHDOG_TIMEOUT;
            loop {
                tokio::time::sleep(sleep_for).await;
                let last_arm = *watchdog_wire
                    .last_arm_at
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                match watchdog_verdict(
                    watchdog_wire.followups_registered.load(Ordering::SeqCst),
                    last_arm,
                    std::time::Instant::now(),
                    PREFIX_WATCHDOG_TIMEOUT,
                ) {
                    WatchdogVerdict::Done => return,
                    WatchdogVerdict::Reschedule(d) => sleep_for = d,
                    WatchdogVerdict::Release => {
                        force_release_prefix_followups(&watchdog_app, &watchdog_wire);
                        sleep_for = PREFIX_WATCHDOG_TIMEOUT;
                    }
                }
            }
        });
    }
}

#[cfg(target_os = "macos")]
fn handle_prefix_followup<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    key: prefix::PrefixKey,
    tab_wire: &Arc<tabs::TabWire>,
    engine: &Engine<R>,
    pause_item: &MenuItem<R>,
) {
    use std::sync::atomic::Ordering;
    let action = {
        let mut st = tab_wire.prefix.lock().unwrap_or_else(|e| e.into_inner());
        st.on_key(std::time::Instant::now(), key)
    };
    tab_wire.prefix_generation.fetch_add(1, Ordering::SeqCst);
    if set_prefix_followups_registered(app, false) {
        tab_wire.followups_registered.store(false, Ordering::SeqCst);
    }
    match action {
        prefix::PrefixAction::Select(tab) => {
            apply_tab_select(app, tab_wire, tab);
        }
        prefix::PrefixAction::PreviousSession | prefix::PrefixAction::NextSession => {
            let agent_selected = {
                let sel = tab_wire
                    .tabs
                    .selection
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                sel.selected() == Some(tabs::Tab::Agent)
            };
            if agent_selected {
                let count = tab_wire.agent_sessions.load(Ordering::Relaxed);
                if count > 0 {
                    let delta: isize = if action == prefix::PrefixAction::NextSession {
                        1
                    } else {
                        -1
                    };
                    let current = tab_wire.viewed_session.load(Ordering::Relaxed) as isize;
                    let next = (current + delta).rem_euclid(count as isize) as usize;
                    tab_wire.viewed_session.store(next, Ordering::Relaxed);
                    tab_wire.session_advanced.notify_one();
                    use tauri::Emitter;
                    if let Err(e) = app.emit(
                        "agent-viewed-session-changed",
                        serde_json::json!({ "index": next }),
                    ) {
                        tracing::error!("failed to emit agent-viewed-session-changed: {e}");
                    }
                }
            }
        }
        prefix::PrefixAction::ExpandToggle => {
            toggle_manual_expand(engine);
        }
        prefix::PrefixAction::TogglePause => {
            toggle_pause(engine, pause_item);
        }
        prefix::PrefixAction::NoOp => {}
    }
}

/// Whether this tick should advance the Agent tab's viewed session automatically: the Agent tab
/// must be selected, there must be 2+ sessions to cycle between.
fn should_auto_advance_session(
    tab_selected: Option<tabs::Tab>,
    session_count: usize,
    hovered: bool,
    paused: bool,
) -> bool {
    tab_selected == Some(tabs::Tab::Agent) && session_count > 1 && !hovered && !paused
}

/// The ONE selection mutation both input paths funnel through — the click monitor calls the same
/// sequence (click.rs). A prefix key and a click must drive identical toggle semantics.
pub(crate) fn apply_tab_select<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    tab_wire: &Arc<tabs::TabWire>,
    tab: tabs::Tab,
) {
    let selected_now = {
        let mut sel = tab_wire
            .tabs
            .selection
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        sel.select(tab);
        sel.selected()
    };
    if selected_now == Some(tabs::Tab::News) {
        tab_wire
            .news_charge
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .visit();
    }
    crate::status::emit_tab_selection_if_transitioned(
        app,
        &tab_wire.tabs.last_emitted,
        selected_now,
    );
}

#[cfg(target_os = "macos")]
fn toggle_manual_expand<R: tauri::Runtime>(engine: &Engine<R>) {
    // expanded changes the rotation window, so the rotation loop's next deadline must be
    // recomputed — apply_blocking wakes it.
    engine.apply_blocking(|q, _now| q.toggle_expanded());
}

#[cfg(target_os = "macos")]
fn dismiss_current<R: tauri::Runtime>(engine: &Engine<R>) {
    engine.apply_blocking(|q, now| q.dismiss_visible(now));
}

#[cfg(target_os = "macos")]
fn skip_current<R: tauri::Runtime>(engine: &Engine<R>) {
    engine.apply_blocking(|q, now| q.skip_visible(now));
}

/// Full parse, never a prefix check: `starts_with("http")` admits `httpx://` (the same trap the
/// settings feed validation already fixed).
#[cfg(target_os = "macos")]
fn openable_http_url(raw: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(raw).ok()?;
    match parsed.scheme() {
        "http" | "https" => Some(parsed.to_string()),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn open_current_story<R: tauri::Runtime>(engine: &Engine<R>) {
    let Some(url) = engine.read_blocking(|q| q.current_link().map(str::to_string)) else {
        tracing::debug!("open story ignored: no visible article link");
        return;
    };

    let Some(normalized) = openable_http_url(&url) else {
        tracing::debug!(%url, "open story ignored: link is not a valid http(s) url");
        return;
    };

    // -u forces URL interpretation (never a file-path fallback), and the argument is the parser's
    // own serialization — what was validated is exactly what executes.
    match std::process::Command::new("/usr/bin/open")
        .arg("-u")
        .arg(&normalized)
        .spawn()
    {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(error) => {
            tracing::debug!(%error, %normalized, "open story command could not be spawned");
        }
    }
}

/// A plain `#[cfg(test)]` module, not part of `mod tests` below: that module compiles out entirely
/// on non-macOS, and the function under test is deliberately not `target_os`-gated.
#[cfg(test)]
mod watchdog_verdict_tests {
    use super::*;
    use std::time::{Duration, Instant};

    const TIMEOUT: Duration = Duration::from_secs(5);

    #[test]
    fn done_when_nothing_is_registered() {
        let t0 = Instant::now();
        assert_eq!(
            watchdog_verdict(false, None, t0, TIMEOUT),
            WatchdogVerdict::Done
        );
        assert_eq!(
            watchdog_verdict(false, Some(t0), t0 + Duration::from_secs(5), TIMEOUT),
            WatchdogVerdict::Done
        );
    }

    #[test]
    fn reschedule_when_a_newer_arm_is_still_inside_its_budget() {
        let t0 = Instant::now();
        let rearmed_at = t0 + Duration::from_secs(4);
        assert_eq!(
            watchdog_verdict(true, Some(rearmed_at), t0 + Duration::from_secs(5), TIMEOUT),
            WatchdogVerdict::Reschedule(Duration::from_secs(4))
        );
    }

    #[test]
    fn reschedule_duration_is_the_newest_arms_remaining_budget() {
        let t0 = Instant::now();
        assert_eq!(
            watchdog_verdict(true, Some(t0), t0 + Duration::from_millis(1), TIMEOUT),
            WatchdogVerdict::Reschedule(Duration::from_millis(4999))
        );
    }

    #[test]
    fn release_when_the_newest_arm_is_exactly_at_its_deadline() {
        let t0 = Instant::now();
        assert_eq!(
            watchdog_verdict(true, Some(t0), t0 + TIMEOUT, TIMEOUT),
            WatchdogVerdict::Release
        );
    }

    #[test]
    fn release_when_the_newest_arm_is_long_past_its_deadline() {
        let t0 = Instant::now();
        assert_eq!(
            watchdog_verdict(true, Some(t0), t0 + Duration::from_secs(60), TIMEOUT),
            WatchdogVerdict::Release
        );
    }

    #[test]
    fn release_when_registered_but_no_arm_was_ever_recorded() {
        let t0 = Instant::now();
        assert_eq!(
            watchdog_verdict(true, None, t0, TIMEOUT),
            WatchdogVerdict::Release
        );
    }

    #[test]
    fn arm_instant_in_the_future_reschedules_for_the_full_budget() {
        let t0 = Instant::now();
        assert_eq!(
            watchdog_verdict(true, Some(t0 + Duration::from_secs(1)), t0, TIMEOUT),
            WatchdogVerdict::Reschedule(TIMEOUT)
        );
    }
}

#[cfg(test)]
mod agent_session_advance_tests {
    use super::*;

    #[test]
    fn should_auto_advance_session_requires_agent_tab_multiple_sessions_no_hover_no_pause() {
        assert!(should_auto_advance_session(
            Some(tabs::Tab::Agent),
            3,
            false,
            false
        ));
    }

    #[test]
    fn should_auto_advance_session_false_when_different_tab_selected() {
        assert!(!should_auto_advance_session(
            Some(tabs::Tab::News),
            3,
            false,
            false
        ));
    }

    #[test]
    fn should_auto_advance_session_false_when_no_tab_selected() {
        assert!(!should_auto_advance_session(None, 3, false, false));
    }

    #[test]
    fn should_auto_advance_session_false_with_one_or_zero_sessions() {
        assert!(!should_auto_advance_session(
            Some(tabs::Tab::Agent),
            1,
            false,
            false
        ));
        assert!(!should_auto_advance_session(
            Some(tabs::Tab::Agent),
            0,
            false,
            false
        ));
    }

    #[test]
    fn should_auto_advance_session_false_while_hovered() {
        assert!(!should_auto_advance_session(
            Some(tabs::Tab::Agent),
            3,
            true,
            false
        ));
    }

    #[test]
    fn should_auto_advance_session_false_while_paused() {
        assert!(!should_auto_advance_session(
            Some(tabs::Tab::Agent),
            3,
            false,
            true
        ));
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::event::{
        test_fixtures, Event, EventSignal, EventType, Priority, RotationSpec, SlotState, SourceKind,
    };

    fn event(priority: Priority) -> Event {
        test_fixtures::with_priority(test_fixtures::event("t"), priority)
    }

    #[test]
    fn cutout_width_js_value_renders_the_number_when_a_cutout_was_reported() {
        let cutout = presentation::CutoutGeometry {
            left_x: 480.5,
            right_x: 799.5,
            width: 319.0,
        };
        assert_eq!(cutout_width_js_value(Some(cutout)), "319");
    }

    #[test]
    fn cutout_width_js_value_renders_null_without_a_cutout() {
        assert_eq!(cutout_width_js_value(None), "null");
    }

    #[test]
    fn cutout_height_js_value_renders_the_inset_when_positive() {
        assert_eq!(cutout_height_js_value(32.0), "32");
    }

    #[test]
    fn cutout_height_js_value_renders_null_at_zero_inset() {
        assert_eq!(cutout_height_js_value(0.0), "null");
    }

    #[test]
    fn cutout_height_js_value_renders_null_for_a_negative_inset() {
        assert_eq!(cutout_height_js_value(-1.0), "null");
    }

    #[test]
    fn board_shrink_runs_when_nothing_moved_since_the_timer_was_armed() {
        let state = BoardFrameState {
            expanded: true,
            generation: 7,
            ..Default::default()
        };
        assert!(board_shrink_should_run(state, 7));
    }

    #[test]
    fn board_shrink_is_cancelled_by_a_re_expand_during_the_grace_period() {
        let state = BoardFrameState {
            expanded: true,
            generation: 8,
            ..Default::default()
        };
        assert!(!board_shrink_should_run(state, 7));
    }

    #[test]
    fn board_shrink_is_a_no_op_once_the_frame_is_already_resting() {
        let state = BoardFrameState {
            expanded: false,
            generation: 7,
            ..Default::default()
        };
        assert!(!board_shrink_should_run(state, 7));
    }

    #[test]
    fn board_shrink_is_cancelled_by_a_second_collapse_request_too() {
        let after_first_request = BoardFrameState {
            expanded: true,
            generation: 1,
            ..Default::default()
        };
        let after_second_request = BoardFrameState {
            expanded: true,
            generation: 2,
            ..Default::default()
        };
        assert!(board_shrink_should_run(after_first_request, 1));
        assert!(!board_shrink_should_run(after_second_request, 1));
        assert!(board_shrink_should_run(after_second_request, 2));
    }

    #[test]
    fn a_fresh_board_frame_state_is_resting_so_a_collapse_is_a_no_op() {
        let state = BoardFrameState::default();
        assert!(!state.expanded);
        assert!(!board_shrink_should_run(state, state.generation));
        assert_eq!(state.height, hover::WINDOW_HEIGHT);
    }

    #[test]
    fn board_collapse_grace_clears_the_disclosure_springs_settle() {
        assert_eq!(BOARD_COLLAPSE_GRACE_MS, 450);
        const { assert!(BOARD_COLLAPSE_GRACE_MS >= 350) };
    }

    fn test_engine(app: &tauri::App<tauri::test::MockRuntime>) -> Engine<tauri::test::MockRuntime> {
        Engine::new(
            SingleSlotQueue::new(50),
            app.handle().clone(),
            false,
            false,
            None,
            std::sync::Arc::new(crate::tabs::TabWire::default()),
        )
    }

    #[test]
    fn toggle_manual_expand_collapses_an_auto_expanded_high_item() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        engine.apply_blocking(|q, now| q.enqueue(event(Priority::High), now).unwrap());

        match engine.read_blocking(|q| q.current_slot_state()) {
            SlotState::Showing { expanded, .. } => {
                assert!(expanded, "High must auto-expand on promotion")
            }
            SlotState::Empty => panic!("expected Showing"),
        }

        toggle_manual_expand(&engine);

        match engine.read_blocking(|q| q.current_slot_state()) {
            SlotState::Showing { expanded, .. } => {
                assert!(!expanded, "hotkey must collapse an auto-expanded High item")
            }
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn toggle_manual_expand_flips_expanded_for_non_high_priority() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        engine.apply_blocking(|q, now| q.enqueue(event(Priority::Medium), now).unwrap());

        toggle_manual_expand(&engine);
        match engine.read_blocking(|q| q.current_slot_state()) {
            SlotState::Showing { expanded, .. } => {
                assert!(
                    !expanded,
                    "first press collapses an auto-expanded Medium item"
                )
            }
            SlotState::Empty => panic!("expected Showing"),
        }

        toggle_manual_expand(&engine);
        match engine.read_blocking(|q| q.current_slot_state()) {
            SlotState::Showing { expanded, .. } => assert!(expanded, "second press re-expands"),
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn dismiss_current_promotes_next_waiting_item() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        engine.apply_blocking(|q, now| q.enqueue(event(Priority::Medium), now).unwrap());
        let next = event(Priority::Medium);
        let next_id = next.id;
        engine.apply_blocking(|q, now| q.enqueue(next, now).unwrap());

        dismiss_current(&engine);

        match engine.read_blocking(|q| q.current_slot_state()) {
            SlotState::Showing { id, .. } => assert_eq!(id, next_id),
            SlotState::Empty => panic!("expected Showing"),
        }
    }

    #[test]
    fn dismiss_current_is_noop_when_slot_already_empty() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        engine.apply_blocking(|q, _now| {
            assert_eq!(q.slot_state_if_changed(), Some(SlotState::Empty));
        });

        dismiss_current(&engine);

        engine.apply_blocking(|q, _now| {
            assert_eq!(q.current_slot_state(), SlotState::Empty);
            assert!(q.slot_state_if_changed().is_none());
        });
    }

    #[test]
    fn skip_current_requeues_recurring_and_promotes_next() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        let mut recurring = event(Priority::Medium);
        recurring.rotation = RotationSpec::Recurring { display_secs: 8 };
        let recurring_id = recurring.id;
        engine.apply_blocking(|q, now| q.enqueue(recurring, now).unwrap());
        let next = event(Priority::Medium);
        let next_id = next.id;
        engine.apply_blocking(|q, now| q.enqueue(next, now).unwrap());

        skip_current(&engine);

        engine.apply_blocking(|q, now| {
            match q.current_slot_state() {
                SlotState::Showing { id, .. } => assert_eq!(id, next_id),
                SlotState::Empty => panic!("expected Showing"),
            }
            assert_eq!(q.total_waiting(), 1);
            q.skip_visible(now);
            match q.current_slot_state() {
                SlotState::Showing { id, .. } => assert_eq!(id, recurring_id),
                SlotState::Empty => panic!("expected recurring item to return"),
            }
        });
    }

    #[test]
    fn openable_http_url_accepts_only_normalized_http_urls() {
        for raw in [
            "https://example.com/a",
            "http://example.com",
            "  https://example.com ",
            "https://exa\tmple.com/pa\nth",
        ] {
            let expected = reqwest::Url::parse(raw).unwrap().to_string();
            assert_eq!(
                openable_http_url(raw),
                Some(expected),
                "should accept and normalize: {raw:?}"
            );
        }

        for raw in [
            "httpx://example.com",
            "file:///etc/hosts",
            "javascript:alert(1)",
            "notaurl",
        ] {
            assert_eq!(openable_http_url(raw), None, "should reject: {raw:?}");
        }
    }

    #[test]
    fn visible_id_from_slot_state_payload_extracts_the_showing_id() {
        let state = SlotState::Showing {
            id: uuid::Uuid::new_v4(),
            title: "t".to_string(),
            body: "b".to_string(),
            event_type: EventType::Generic,
            priority: Priority::Medium,
            signal: EventSignal::Generic,
            origin: SourceKind::Manual,
            expanded: false,
            source: None,
            category: None,
            published_at_ms: None,
            link: None,
            subtitle: None,
            details: Vec::new(),
            queue_total: 1,
            queue_done: 0,
            ttl_ms: 8000,
            remaining_ms: 8000,
            espn: None,
            agent_runtime: None,
        };
        let SlotState::Showing { id, .. } = &state else {
            unreachable!()
        };
        let expected = id.to_string();
        let payload = serde_json::to_string(&state).unwrap();

        assert_eq!(visible_id_from_slot_state_payload(&payload), Some(expected));
    }

    #[test]
    fn visible_id_from_slot_state_payload_is_none_for_empty() {
        let payload = serde_json::to_string(&SlotState::Empty).unwrap();
        assert_eq!(visible_id_from_slot_state_payload(&payload), None);
    }

    #[test]
    fn visible_id_from_slot_state_payload_is_none_for_malformed_json() {
        assert_eq!(visible_id_from_slot_state_payload("not json"), None);
        assert_eq!(visible_id_from_slot_state_payload(""), None);
        assert_eq!(visible_id_from_slot_state_payload("{}"), None);
    }

    #[test]
    fn escape_for_osascript_escapes_backslash_and_quote() {
        assert_eq!(
            escape_for_osascript(r#"bad key "port" at line 3"#),
            r#"bad key \"port\" at line 3"#
        );
        assert_eq!(escape_for_osascript(r"C:\config"), r"C:\\config");
    }

    #[test]
    fn escape_for_osascript_leaves_plain_text_untouched() {
        assert_eq!(
            escape_for_osascript("config.toml is malformed (missing field)"),
            "config.toml is malformed (missing field)"
        );
    }

    #[test]
    fn open_current_story_is_noop_without_visible_link() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        engine.apply_blocking(|q, _now| {
            assert_eq!(q.slot_state_if_changed(), Some(SlotState::Empty));
        });

        open_current_story(&engine);

        engine.apply_blocking(|q, _now| {
            assert_eq!(q.current_slot_state(), SlotState::Empty);
            assert!(q.slot_state_if_changed().is_none());
        });
    }

    #[test]
    fn script_close_tag_cannot_survive() {
        let escaped = escape_for_eval_splice(r#"{"title":"x</script><script>"}"#);
        assert!(
            !escaped.contains('<'),
            "no literal `<` may survive: {escaped}"
        );
        assert!(escaped.contains("\\u003c/script>\\u003cscript>"));
    }

    #[test]
    fn line_separators_escaped() {
        let input = "a\u{2028}b\u{2029}c";
        let escaped = escape_for_eval_splice(input);
        assert!(escaped.contains("\\u2028"));
        assert!(escaped.contains("\\u2029"));
        assert!(!escaped.contains('\u{2028}'));
        assert!(!escaped.contains('\u{2029}'));
    }

    #[test]
    fn round_trips_as_json() {
        let title = "goal </script> \u{2028}\u{2029}end";
        let state = SlotState::Showing {
            id: uuid::Uuid::new_v4(),
            title: title.to_string(),
            body: "b".to_string(),
            event_type: EventType::Generic,
            priority: Priority::Medium,
            signal: EventSignal::Generic,
            origin: SourceKind::Manual,
            expanded: false,
            source: None,
            category: None,
            published_at_ms: None,
            link: None,
            subtitle: None,
            details: Vec::new(),
            queue_total: 1,
            queue_done: 0,
            ttl_ms: 8000,
            remaining_ms: 8000,
            espn: None,
            agent_runtime: None,
        };
        let escaped = escape_for_eval_splice(&serde_json::to_string(&state).unwrap());

        let parsed: serde_json::Value =
            serde_json::from_str(&escaped).expect("escaped output must still parse as JSON");
        assert_eq!(parsed["title"].as_str().unwrap(), title);
    }
}
