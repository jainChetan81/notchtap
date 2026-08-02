mod about;
// v7 (plan 133/134/138): provider-neutral Agent domain model + registry
// (`model.rs`/`registry.rs`, plan 133) plus wire parsing and the
// `/agent/events` route (`adapter.rs`, plan 134) — see `agents/mod.rs`'s
// doc for the ticket boundary. `pub` (plan 138, otherwise this module's
// comment above `error`/`event`/`queue` would still hold: "nothing else
// consumes this crate as a library") is the one-line exception those
// three already carved out — the `notchtap-agent` bin target
// (`src/bin/notchtap_agent.rs`, its own separate crate within this same
// package) calls `agents::providers::*` and `agents::adapter::*`, which
// is unreachable across a crate boundary while this stays a private
// `mod`. Nothing under `agents` gained new internal-to-this-crate
// visibility — every item this exposes was already `pub` within the
// crate; only the outermost `mod` keyword changed.
pub mod agents;
mod config;
mod crests;
mod engine;
// queue, event, and error are `pub` so their doc-tests can exercise the
// real public api (doc-tests link against the lib crate from outside);
// nothing else consumes this crate as a library.
pub mod error;
pub mod event;
mod history;
mod hover;
mod http;
mod logging;
#[cfg(target_os = "macos")]
mod login_item;
mod net;
mod notifier;
mod now_playing;
mod poller;
mod presentation;
pub mod queue;
mod rss_poller;
mod settings;
// The single source of truth for the eighteen settings-window commands
// (see this module's own doc comment) — build.rs's AppManifest::commands
// allowlist, the generate_handler![...] registration just below, and
// capabilities/settings.json must all name exactly the commands listed
// there. Its own tests are the parity guard for that triple.
mod settings_commands;
pub mod silence;
mod status;
mod weather_poller;

use std::sync::{Arc, Mutex as StdMutex, Once, OnceLock};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
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

#[cfg(target_os = "macos")]
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

// plan 045: tauri-nspanel v2.1's to_panel() requires an explicit panel
// type. can_become_key_window: true preserves the pinned rev's behavior
// (RawNSPanel hardcoded canBecomeKeyWindow -> YES); can_become_main_window:
// false matches NSPanel's AppKit default, which the pinned rev never
// overrode.
//
// plan 087: `with: { tracking_area: {...} }` attaches a real
// NSTrackingArea to the panel's content view — this is what makes
// mouseEntered/mouseMoved/mouseExited observable at all. Empirically
// verified (docs/design/hover-cursor-tracking.md §2) to fire normally
// even with `set_ignore_cursor_events(true)` (apply_overlay_native_config,
// below) permanently set — that call is NEVER made conditional on this.
// `active_always()` is required (not `active_in_active_app()`): this is
// a non-activating accessory panel, so hover must be observable while
// another app is focused. `auto_resize: true` mirrors the upstream
// examples but is a no-op here since the window frame itself never
// resizes (`tauri.conf.json`'s `"resizable": false`) — only the CSS
// width within it changes; that's the reason `hover::active_card_rect`
// exists at all (see its doc comment).
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

// ⌃⇧] / ⌃⇧, — chosen (and already shipped in the settings UI's shortcut
// table) to avoid the four combos above and common macOS ⌘-based
// shortcuts, same rule as ⌃⇧X/⌃⇧P (v3.6 spec §7.1.2).
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

// plan 144 (v7 ticket 12 of 13, spec §6.3): the Open/Focus Session
// shortcut — ⌃⇧A, chosen by the same "avoid the combos already listed
// above and common ⌘-based shortcuts" rule as SKIP/OPEN_SETTINGS.
#[cfg(target_os = "macos")]
const FOCUS_SESSION_SHORTCUT: (Option<Modifiers>, Code) =
    (Some(Modifiers::CONTROL.union(Modifiers::SHIFT)), Code::KeyA);

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
            // this process is a login item with no terminal attached in
            // the normal case — `eprintln!`/`tracing::error!` above are
            // both invisible to a user who launched it from the Dock or
            // at login. A blocking native dialog is the only way this
            // failure is ever actually seen, so it must be shown BEFORE
            // the exit below, not logged only.
            show_boot_error_dialog(&format!(
                "notchtap couldn't start: config.toml is malformed ({e})"
            ));
            std::process::exit(1);
        }
    };

    // Boot-time contract parity with the settings window (plan 013): the
    // file is the other editing surface, so it gets the same validation —
    // but warn-and-continue, not exit: a range violation must not brick an
    // always-on login item. Malformed TOML still fails fast in Config::load.
    if let Err(violations) = crate::settings::validate(&config) {
        for v in &violations {
            tracing::warn!(violation = %v, "config.toml value out of range — running with it anyway");
        }
    }

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
    // plan 037: the bare queue moves into `setup`, where Engine::new takes
    // it BY VALUE and creates the wake and live-match handle internally —
    // after that, no code outside engine.rs can hold any of the three.
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
    let espn_live_card = config.espn_live_card;
    let espn_rich_events = config.espn_rich_events;
    // plan 083 workstream a: `~/.config/notchtap/crests/`, a sibling of
    // config.toml/secrets.toml under the same directory
    // (`Config::dir_from_home`) — the repo's first binary-asset cache.
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
    // v7 (plan 137, spec §7): `[agents]` config drives both the Agent
    // Registry's stale/retention durations (below, at registry
    // construction) and the `agent_events_handler`'s `NotificationPolicy`/
    // per-runtime gate (`http::AppState`, further down in `setup`).
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
    // Operator decision 2026-08-02: the Agent Board's PRESENCE gate,
    // applied in exactly one place — `AgentBoardPublisher::gate_presence`
    // (agents/board.rs). Nothing downstream of the publisher (the
    // overlay's `presentationMode`, the hover-expand path below) knows
    // this flag exists; they all read the published snapshot instead.
    let agent_board_show_working = agents_config.board_show_working;
    let agent_stale_after = std::time::Duration::from_secs(agents_config.stale_after_secs);
    let agent_terminal_retention =
        std::time::Duration::from_secs(agents_config.terminal_retention_secs);
    let agent_stale_retention = std::time::Duration::from_secs(agents_config.stale_retention_secs);
    let weather_enabled = config.weather_enabled;
    let weather_lat = config.weather_lat;
    let weather_lon = config.weather_lon;
    let weather_units = config.weather_units;
    let weather_poll_secs = config.weather_poll_secs;
    let weather_rain_threshold_pct = config.weather_rain_threshold_pct;
    let weather_rain_lookahead_mins = config.weather_rain_lookahead_mins;
    let weather_temp_hot_c = config.weather_temp_hot_c;
    let weather_temp_cold_c = config.weather_temp_cold_c;
    let weather_ttl_secs = config.weather_ttl_secs;
    let weather_priority = config.weather_priority;
    let history_enabled = config.history_enabled;
    let now_playing_enabled = config.now_playing_enabled;
    let now_playing_adapter_enabled = config.now_playing_adapter_enabled;
    let now_playing_adapter_dir = config.now_playing_adapter_dir.clone();
    // plan 146a: the `[silence]` block feeds `SilenceController::new` at
    // boot (below, in `setup`) — session-only mute/skip state is never
    // read from config, only the daily schedule.
    let silence_schedule_enabled = config.silence.enabled;
    let silence_window = config.silence.window;

    // v3 outbound connectors: built here (channel needs no runtime), any
    // worker future would be spawned in setup once the runtime exists. no
    // connectors are wired up currently — the fan-out framework
    // (`ConnectorHandle`) stays in place for a future connector (plan 128).
    let connector_handles: Vec<notifier::ConnectorHandle> = Vec::new();
    let connectors = Arc::new(connector_handles);
    let server_once = Arc::new(Once::new());

    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    builder
        // v5 settings commands (settings.rs) — every one of these is also
        // listed in build.rs's AppManifest::commands; that pairing is what
        // keeps them deniable to the overlay window (spec §2).
        .invoke_handler(tauri::generate_handler![
            settings::clear_history,
            settings::clear_queue,
            settings::get_config,
            settings::get_default_config,
            settings::get_history,
            settings::get_queue,
            settings::get_recent_log_lines,
            settings::get_secret_status,
            settings::save_config_and_relaunch,
            settings::search_news_now,
            settings::set_secret,
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
            // About section (plan: get_about_info) reports process uptime
            // from this, not system uptime — captured once, here, before
            // anything else in setup can meaningfully delay boot.
            app.manage(std::time::Instant::now());
            app.manage(StdMutex::new(config_for_state));
            // plan 130: the ONE SeenStore both the rss poller loop (below)
            // and the settings window's `search_news_now` one-shot command
            // dedup against — app-managed state (same mechanism as Config
            // above), reached by the poller via its AppHandle and by the
            // command via `tauri::State`. Always managed regardless of
            // `rss_enabled`: an ad-hoc search should work even with
            // continuous polling off.
            app.manage(StdMutex::new(rss_poller::SeenStore::default()));
            // plan 130: serializes concurrent `search_news_now` calls — a
            // second call while one is in flight errors "already
            // searching" rather than racing the same SeenStore/http
            // client (settings.rs's own doc comment on the command has
            // the full rationale).
            app.manage(std::sync::atomic::AtomicBool::new(false));
            // plan 037: the ONE Engine. By-value construction means `run()`
            // holds no queue/wake/live binding after this line — a retained
            // alias is a compile error, not a convention. Managed as state
            // so the settings commands (send_test_notification) and the
            // on_page_load/server_once closures below can reach the same
            // Engine the rotation loop and pollers run on.
            // plan 088: `None` when disabled (the default) — the Engine's
            // hook is then a no-op and behavior is byte-identical to
            // pre-088. A store that fails to open (unwritable config dir)
            // degrades to `None` with a warning rather than failing boot;
            // history is a convenience, not a correctness requirement.
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
            let engine = Engine::new(
                initial_queue,
                app.handle().clone(),
                connectors.clone(),
                espn_enabled,
                rss_enabled,
                weather_enabled,
                now_playing_enabled,
                history,
            );
            app.manage(engine.clone());

            // plan 146a: the one SilenceController, managed the same way as
            // `engine` above (an `Arc<StdMutex<_>>` rather than the
            // Engine's own `Arc<Mutex<_>>` because this is read/mutated
            // from the tray's main-thread handlers as well as the async
            // schedule task below — `StdMutex` avoids requiring an async
            // context just to check a mute deadline). Session-only
            // skip/mute state starts empty every boot by construction
            // (`SilenceController::new` takes no such state); only the
            // daily schedule comes from config.
            let silence_controller = Arc::new(StdMutex::new(silence::SilenceController::new(
                silence_schedule_enabled,
                silence_window,
            )));
            app.manage(silence_controller.clone());

            // v7 (plan 133/134/137): the one Agent Registry, managed exactly
            // like `engine` above so both the HTTP layer (`http::AppState`,
            // below) and later tickets (IPC, settings) can reach the same
            // instance via `AppHandle::state`. `stale_after`/
            // `terminal_retention`/`stale_retention` now come from real
            // `[agents]` config (`agent_stale_after`/`agent_terminal_retention`/
            // `agent_stale_retention`, hoisted above from
            // `config.agents.stale_after_secs`/`terminal_retention_secs`/
            // `stale_retention_secs`) rather than the spec-default
            // hardcodes plan 134 shipped with.
            let agent_registry = agents::registry::AgentRegistryHandle::new(
                agents::registry::AgentRegistry::new(
                    agent_stale_after,
                    agent_terminal_retention,
                    agent_stale_retention,
                ),
            );
            app.manage(agent_registry.clone());

            // Plan 143 (v7 ticket 11 of 13, spec §4.6/§8/§10): the shared
            // Adapter Health tracker — managed exactly like
            // `agent_registry` above so `server_once`'s `http::AppState`
            // (below), `agent_board`'s own publish path, and the Settings
            // `get_agent_health` command all reach the same instance,
            // never independent copies (an `Arc` handle, same "one
            // instance" discipline `AgentBoardPublisher`'s doc gives for
            // its own dedup bookkeeping).
            let agent_health = std::sync::Arc::new(agents::health::HealthTracker::new());
            app.manage(agent_health.clone());

            // v7 (plan 136, spec §6): the `agent-state` IPC publisher —
            // managed the same way as `engine`/`agent_registry` above so
            // `server_once`'s `http::AppState` (below) can reach the same
            // instance the periodic tick (`spawn_tick`, right after) also
            // publishes through; both call sites must share one
            // dedup/revision bookkeeping instance, never two independent
            // ones (see `AgentBoardPublisher`'s own doc for why).
            let agent_board = agents::board::AgentBoardPublisher::new(
                app.handle().clone(),
                agent_registry,
                agent_health.clone(),
                agent_runtimes,
                agent_board_show_working,
            );
            app.manage(agent_board.clone());
            agent_board.spawn_tick(agents::board::DEFAULT_TICK_INTERVAL);

            let window = app
                .get_webview_window("main")
                .expect("main window missing from tauri.conf.json");
            window.set_always_on_top(true)?;

            // plan 097: hoisted out of the tracking-area block below so the
            // global-shortcut handler (registered further down, in its own
            // `#[cfg(target_os = "macos")]` block) can also reach it — the
            // dismiss/skip hotkeys replace the visible card with no mouse
            // event firing, so they must force this latch back to "not
            // hovered" themselves (see `emit_hover_changed_if_transitioned`
            // call sites in the shortcut handler).
            #[cfg(target_os = "macos")]
            let was_hovered = Arc::new(StdMutex::new(false));

            // plan 142 (v7 ticket 10 of 13, spec §6.2 expanded): whether
            // the Agent Board's window frame is CURRENTLY the expanded
            // one (and pointer delivery is temporarily enabled) — set the
            // instant a board hover-entry expands it, cleared the instant
            // a hover-exit (or any other `emit_hover_changed_if_transitioned`
            // call site going false) collapses it back. Gates the
            // collapse path so a hover-exit over a NON-board card (which
            // never expanded anything) doesn't do needless window-frame
            // churn.
            #[cfg(target_os = "macos")]
            let board_expanded = Arc::new(StdMutex::new(false));

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
                    .to_panel::<OverlayPanel>()
                    .map_err(|e| format!("nspanel conversion failed: {e:?}"))?;
                // NSWindowStyleMaskNonactivatingPanel (1 << 7); the window
                // is borderless (mask 0), so the panel bit is the whole mask.
                panel.set_style_mask(objc2_app_kit::NSWindowStyleMask::NonactivatingPanel);

                // plan 087: the hover primitive. `set_ignore_cursor_events
                // (true)` (apply_overlay_native_config, below) is NEVER
                // touched by any of this — the tracking area's mouseEntered/
                // mouseMoved/mouseExited fire independent of click-through,
                // empirically verified (docs/design/hover-cursor-tracking.md
                // §2). `hover-changed` is emitted ONLY when the hovered
                // boolean flips (emit_hover_changed_if_transitioned), never
                // per mouse-move.
                let hover_handler = OverlayPanelEventHandler::new();
                let hover_cutout_width = cutout.map(|c| c.width).unwrap_or(0.0);
                // plan 093: the y-span fix's cutout-HEIGHT term. Mirrors
                // `cutout_height_js_value`'s own reasoning (lib.rs, near
                // the on_page_load eval-splice site) — `CutoutGeometry`
                // carries no height field, so `inset`
                // (`DetectOutput::safe_area_top_inset`, already
                // destructured above and already in scope here) is the
                // notch's real height in notch mode; `0.0` in HUD mode,
                // where `hover::active_card_rect` ignores this argument
                // entirely in favor of `HUD_CUTOUT_H` anyway (same
                // pattern `hover_cutout_width` already follows).
                let hover_cutout_height = inset;

                {
                    let engine = engine.clone();
                    let app_handle = app.handle().clone();
                    let was_hovered = was_hovered.clone();
                    let agent_board = agent_board.clone();
                    let board_expanded = board_expanded.clone();
                    let window = window.clone();
                    hover_handler.on_mouse_entered(move |event| {
                        let loc = event.locationInWindow();
                        // plan 093: read BEFORE this event can overwrite
                        // it — `hover_point_is_over_card`'s own doc
                        // explains why the CURRENT (pre-event) value is
                        // the correct hysteresis input for whether the
                        // idle peek's rect should already be grown.
                        let idle_peek_open = *was_hovered.lock().unwrap();
                        let hovered = hover_point_is_over_card(
                            &engine,
                            &app_handle,
                            mode,
                            hover_cutout_width,
                            hover_cutout_height,
                            idle_peek_open,
                            agent_board.last_session_count(),
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
                            &board_expanded,
                        );
                    });
                }
                {
                    let engine = engine.clone();
                    let app_handle = app.handle().clone();
                    let was_hovered = was_hovered.clone();
                    let agent_board = agent_board.clone();
                    let board_expanded = board_expanded.clone();
                    let window = window.clone();
                    hover_handler.on_mouse_moved(move |event| {
                        let loc = event.locationInWindow();
                        let idle_peek_open = *was_hovered.lock().unwrap();
                        let hovered = hover_point_is_over_card(
                            &engine,
                            &app_handle,
                            mode,
                            hover_cutout_width,
                            hover_cutout_height,
                            idle_peek_open,
                            agent_board.last_session_count(),
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
                            &board_expanded,
                        );
                    });
                }
                {
                    let engine = engine.clone();
                    let app_handle = app.handle().clone();
                    let was_hovered = was_hovered.clone();
                    let agent_board = agent_board.clone();
                    let board_expanded = board_expanded.clone();
                    let window = window.clone();
                    // Leaving the window's tracking area is never "still
                    // hovered" regardless of where the cursor lands next —
                    // no rect comparison needed.
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
                            &board_expanded,
                        );
                    });
                }

                panel.set_event_handler(Some(hover_handler.as_ref()));

                // Generic hover-latch reset (M5): plan 097 reset
                // `was_hovered` back to false after the dismiss/skip
                // hotkeys specifically, because those replace the visible
                // card with no mouse event firing to trip the
                // transitions-only gate naturally. That fix enumerated
                // caller sites and missed two others that also replace the
                // visible item with no mouse event: an idle-peek card
                // promoting under an already-hovering cursor (the idle
                // rect and the new Showing rect can both contain the
                // cursor, so `hovered` reads true before AND after the
                // promotion — the gate sees no transition and
                // `hover_enter` never fires for the new item, so the TTL
                // hover-pause stays dead and the card rotates out under a
                // moving cursor), and the settings window's `skip_current`
                // command (which mutates through `Engine::apply`, nowhere
                // near this AppKit event handler at all).
                //
                // Rather than chase every current and future caller that
                // can change the visible item, listen for the one channel
                // EVERY such change already flows through regardless of
                // origin: the `slot-state` wire event itself. Whenever the
                // emitted item's id differs from the last one observed,
                // force `was_hovered` back to false — the transitions-only
                // gate then treats the cursor's hover state as unknown
                // again, so the next real mouse-move (the cursor is, per
                // the bug report, already moving in the case this exists
                // to fix) recomputes fresh for the new item instead of
                // staying latched true from the old one and never firing.
                // Same accepted residual as plan 097's resets: a perfectly
                // stationary cursor doesn't recompute until it moves 1px.
                {
                    use tauri::Listener;
                    let was_hovered = was_hovered.clone();
                    let last_visible_id: Arc<StdMutex<Option<String>>> =
                        Arc::new(StdMutex::new(None));
                    // plan 142: also clone in the board-collapse inputs — a
                    // new Notification taking the Slot (a real `id` arriving
                    // here) means the overlay's own `presentationMode`
                    // switches away from the Board entirely (spec §6.1's
                    // precedence: Visible Notification always wins), so any
                    // still-expanded Board window frame must collapse right
                    // alongside the existing hover-latch reset — this path
                    // deliberately never emits `hover-changed` (see the
                    // paragraph above), so it can't route through
                    // `emit_hover_changed_if_transitioned` itself; it calls
                    // the same idempotent collapse helper directly instead.
                    let board_expanded = board_expanded.clone();
                    let window = window.clone();
                    app.handle()
                        .listen(crate::event::SLOT_STATE_EVENT, move |event| {
                            let new_id = visible_id_from_slot_state_payload(event.payload());
                            let is_real_notification = new_id.is_some();
                            let mut last =
                                last_visible_id.lock().unwrap_or_else(|e| e.into_inner());
                            if *last != new_id {
                                *last = new_id;
                                *was_hovered.lock().unwrap_or_else(|e| e.into_inner()) = false;
                                if is_real_notification {
                                    collapse_board_if_expanded(&window, mode, cutout, &board_expanded);
                                }
                            }
                        });
                }
            }

            // v3.6 spec §7.2: survive Spaces switches and fullscreen apps.
            #[cfg(target_os = "macos")]
            apply_overlay_native_config(&window)?;

            position_window(&window, mode, cutout)?;
            let (pause_item, silenced_indicator_item) = build_tray(
                app.handle(),
                engine.clone(),
                start_paused,
                silence_controller.clone(),
            )?;

            // v3.6 spec §7.1: manual expand toggle, rust-side only — the
            // frontend never calls the plugin's JS api (receive-only
            // boundary, unchanged), so no capabilities/permissions entry
            // is needed for this.
            #[cfg(target_os = "macos")]
            {
                let engine_for_handler = engine.clone();
                let pause_item_for_handler = pause_item.clone();
                let was_hovered_for_handler = was_hovered.clone();
                let agent_board_for_handler = agent_board.clone();
                let board_expanded_for_handler = board_expanded.clone();
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
                                    // plan 097: the dismiss hotkey replaces the
                                    // visible card with no mouse event firing, so
                                    // the AppKit-side latch never resets on its
                                    // own and the transitions-only gate then
                                    // swallows the cursor's re-enter onto the new
                                    // card — force it back to "not hovered" here;
                                    // the next real mouse move re-enters normally.
                                    // Accepted residual: a perfectly stationary
                                    // cursor stays un-paused until it moves 1px.
                                    emit_hover_changed_if_transitioned(
                                        &engine_for_handler,
                                        app,
                                        &was_hovered_for_handler,
                                        false,
                                        &window_for_handler,
                                        mode,
                                        cutout,
                                        &agent_board_for_handler,
                                        &board_expanded_for_handler,
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
                                    // plan 097: same hover-latch desync as the
                                    // dismiss arm above — the skip hotkey also
                                    // replaces the visible card with no mouse
                                    // event.
                                    emit_hover_changed_if_transitioned(
                                        &engine_for_handler,
                                        app,
                                        &was_hovered_for_handler,
                                        false,
                                        &window_for_handler,
                                        mode,
                                        cutout,
                                        &agent_board_for_handler,
                                        &board_expanded_for_handler,
                                    );
                                } else if *shortcut
                                    == Shortcut::new(
                                        OPEN_SETTINGS_SHORTCUT.0,
                                        OPEN_SETTINGS_SHORTCUT.1,
                                    )
                                {
                                    open_settings_window(app);
                                } else if *shortcut
                                    == Shortcut::new(
                                        FOCUS_SESSION_SHORTCUT.0,
                                        FOCUS_SESSION_SHORTCUT.1,
                                    )
                                {
                                    // plan 144 (spec §6.3): Rust-only, no
                                    // overlay involvement — the overlay
                                    // stays receive-only. The registry
                                    // lock is async, so the lookup +
                                    // activation runs on the async
                                    // runtime rather than blocking this
                                    // (synchronous) shortcut callback.
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
            }

            #[cfg(target_os = "macos")]
            login_item::register();
            // v6: polling is enabled/disabled once at boot from Config and
            // never flipped again (no longer tray-toggleable — the tray's
            // "Pause Football Scores"/"Pause News" items were redundant
            // with the settings panel's espn_enabled/rss_enabled toggles,
            // ARCHITECTURE.md §17's "richer than a toggle lives in
            // Settings" rule). Each poller below simply doesn't spawn when
            // its `_enabled` flag is false.
            // plan 037: the rotation loop (formerly spawn_heartbeat) lives
            // inside the Engine — it is the consumer of the wake, so the
            // wake never escapes engine.rs.
            engine.spawn_rotation();

            // plan 146a: the Silenced schedule/mute timer — always spawned
            // (unlike the pollers below, which are config-gated) because
            // even a disabled schedule can still have a tray mute started
            // against it; the task itself is what makes a disabled
            // schedule with no mute running a permanent no-op.
            spawn_silence_task(
                engine.clone(),
                silence_controller.clone(),
                silenced_indicator_item,
            );

            // espn poller (v2 spec §3) — config-gated: `espn_enabled =
            // false` means it never spawns. first poll only baselines
            // (silent), so starting before the webview loads can't drop
            // anything a listener would have shown.
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
                );
            }

            // weather poller (plan 040 Part B) — config-gated the same
            // way: `weather_enabled = false` (the default) means it never
            // spawns and the idle rail shows no weather chip.
            if weather_enabled {
                weather_poller::spawn_weather_poller(
                    engine.clone(),
                    weather_lat,
                    weather_lon,
                    weather_units,
                    weather_poll_secs,
                    weather_rain_threshold_pct,
                    weather_rain_lookahead_mins,
                    weather_temp_hot_c,
                    weather_temp_cold_c,
                    weather_ttl_secs,
                    weather_priority,
                );
            }

            // now-playing ambient source (plan 104) — config-gated by
            // BOTH the feature toggle and the kill switch; the module's
            // own spawn function additionally requires the vendored
            // adapter's two files to exist at `now_playing_adapter_dir`
            // before starting the child (clean degrade, one warn-level
            // log, never a startup error — mirrors `detect_path`'s own
            // missing-binary tolerance).
            now_playing::spawn_now_playing_poller(
                engine.clone(),
                now_playing_enabled,
                now_playing_adapter_enabled,
                now_playing_adapter_dir,
            );

            Ok(())
        })
        .on_page_load(move |webview, payload| {
            // listener-ready gate (spec §3): tauri events are transient, so
            // the /notify listener binds only once the webview has loaded
            // and its `notification-promoted` listener can exist. before
            // this, the cli gets connection-refused — honest, not a silent
            // 200-drop.
            if payload.event() == PageLoadEvent::Finished && webview.label() == "main" {
                let app_handle = webview.app_handle().clone();
                // plan 037: retrieve the ONE Engine via managed state —
                // this closure is built before `setup` runs, so it cannot
                // capture the Engine; a second Engine::new here would
                // create a second wake AND a second live-match handle no
                // rotation loop waits on or writes to (the exact
                // stall/desync class 015/036 fixed).
                let engine = app_handle.state::<Engine>().inner().clone();

                // slot-state is double-shielded against the
                // listener-registration race (2026-07-17 review, this
                // migration's own fix): the eval plants a global that react
                // reads as *initial* state if it mounts after this moment;
                // the emit reaches the listener if react mounted before it.
                // one of the two always lands, and running on every page
                // load (not once) covers reloads too — which is why the
                // emit is UNCONDITIONAL (dedup deliberately bypassed).
                // blocking_lock is safe here, same as the tray menu
                // handler below: this callback runs off the tokio runtime,
                // not on it.
                //
                // Ordering fix: the global is planted BEFORE the wire emit
                // fires, not after. Emitting first left a real gap — a
                // webview that finishes mounting its `slot-state` listener
                // between the emit and the eval call would see neither (the
                // emit already fired with no listener yet, and the global
                // isn't set yet either), landing on `undefined` until the
                // next real content change. Planting first means the two
                // now overlap instead: a late-mounting react reads the
                // global either way, and an already-mounted listener still
                // gets the emit a moment later — the frontend's own dedup
                // is what makes that harmless double-land a no-op.
                // `current_slot_state_blocking`/`status_snapshot_blocking`
                // (engine.rs) are the non-emitting halves that make this
                // ordering possible; `emit_slot_state`/`emit_status_state`
                // below are then called explicitly, after the eval.
                {
                    let current_state = engine.current_slot_state_blocking();
                    let state_json =
                        serde_json::to_string(&current_state).unwrap_or_else(|_| "null".into());
                    let safe_json = escape_for_eval_splice(&state_json);
                    let _ = webview.eval(format!("window.__NOTCHTAP_SLOT_STATE__ = {safe_json};"));
                    crate::event::emit_slot_state(&app_handle, current_state);
                }

                // plan 034: the status rail gets the identical dual-path
                // race shield — eval-planted global for late-mounting
                // react, one emit for an already-registered listener, same
                // escaping helper, same plant-before-emit ordering as the
                // slot-state block above.
                {
                    let current_status = engine.status_snapshot_blocking();
                    let status_json =
                        serde_json::to_string(&current_status).unwrap_or_else(|_| "null".into());
                    let safe_json = escape_for_eval_splice(&status_json);
                    let _ =
                        webview.eval(format!("window.__NOTCHTAP_STATUS_STATE__ = {safe_json};"));
                    crate::status::emit_status_state(&app_handle, current_status);
                }

                // Double-shield the initial appearance values the same way as
                // slot state above: a global for the React mount race, plus
                // an emit for listeners already registered.
                {
                    use tauri::Emitter;
                    // plan 085: the seed must carry resting_state too — a
                    // fresh boot learns the flag ONLY from this seed, so
                    // building the payload from the whole Config (not just
                    // Appearance) is required, not optional.
                    let config = app_handle.state::<StdMutex<Config>>().lock().unwrap().clone();
                    let payload = AppearanceChangedPayload::from_config(&config);
                    let payload_json = escape_for_eval_splice(
                        &serde_json::to_string(&payload).unwrap_or_else(|_| "null".into()),
                    );
                    let _ =
                        webview.eval(format!("window.__NOTCHTAP_APPEARANCE__ = {payload_json};"));
                    let _ = webview.emit("appearance-changed", &payload);
                }

                // plan 063: presentation facts for the frontend — the mode boolean and
                // the numeric cutout width, one eval, same page-load site as the other
                // boot facts. plan 060 will consume __NOTCHTAP_MODE__ when it lands.
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

/// Blocking native error dialog for boot-time failures that happen
/// BEFORE any window (or even the tauri runtime) exists — `Config::load`
/// failing is the only caller today. This process is normally a login
/// item with no attached terminal, so `tracing::error!`/`eprintln!` are
/// invisible to the user; without this, a malformed `config.toml` looks
/// like the app silently refusing to launch, with no way to learn why.
///
/// No new dependency: neither `rfd` nor `tauri-plugin-dialog` is in
/// Cargo.toml, and adding one is out of scope here (see this fix's own
/// instructions) — `osascript` is a system binary already reachable via
/// `std::process::Command`, the same mechanism `open_current_story`
/// already uses to shell out to `/usr/bin/open`. `.status()` blocks this
/// thread until the dialog is dismissed, which is exactly what "shown
/// BEFORE the exit" requires — `run()` calls this and then
/// `std::process::exit(1)` immediately after, so there is nothing else
/// for this thread to do in the meantime anyway.
#[cfg(target_os = "macos")]
fn show_boot_error_dialog(message: &str) {
    let script = format!(
        "display dialog \"{}\" with title \"notchtap\" buttons {{\"Quit\"}} default button \"Quit\" with icon stop",
        escape_for_osascript(message)
    );
    // best-effort: if osascript itself can't be spawned (e.g. a stripped
    // CI sandbox with no Foundation/Carbon frameworks reachable), the
    // process still exits below via the caller's own std::process::exit —
    // this dialog is a courtesy, not the actual failure signal.
    let _ = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(&script)
        .status();
}

#[cfg(not(target_os = "macos"))]
fn show_boot_error_dialog(_message: &str) {}

/// Escapes a string for embedding inside an AppleScript double-quoted
/// string literal (the `display dialog "..."` argument above): backslash
/// and double-quote are AppleScript's own escape-needing characters
/// inside a quoted string, same rule as any shell-adjacent quoting —
/// config-load error text can contain arbitrary path/TOML-parser text
/// (e.g. a quoted TOML key), so this must not be skipped.
#[cfg(target_os = "macos")]
fn escape_for_osascript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Makes a serde_json string safe to splice into eval'd JS source:
/// payloads may carry arbitrary caller text (espn scoring-play strings,
/// agent titles — superseded the earlier cmux relay, plan 137).
/// U+2028/U+2029 are legal in JSON but illegal raw in JS
/// source, and `<` closes the gap JSON leaves (it doesn't escape `/`,
/// so a literal "</script>" would otherwise break out of the script
/// context).
fn escape_for_eval_splice(json: &str) -> String {
    json.replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
        .replace('<', "\\u003c")
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

// plan 063: the cutout width as a JS literal for the page-load eval splice
// — a positive JSON number when the shim reported a cutout, `null`
// otherwise (hud mode, or an older/zero-width report). `width <= 0.0`
// cannot occur here: `presentation::DetectOutput::cutout()` normalizes it
// to `None` upstream.
fn cutout_width_js_value(cutout: Option<presentation::CutoutGeometry>) -> String {
    match cutout {
        Some(c) => format!("{}", c.width),
        None => "null".into(),
    }
}

// plan 091: the notch cutout's HEIGHT, exposed through the same eval-splice
// site as its width, one line below the width's own splice. `CutoutGeometry`
// carries no height field (it's purely the horizontal bounds the shim
// reports) — the height is `DetectOutput::safe_area_top_inset`
// (presentation.rs:41), already destructured into `inset` at this
// function's top (`detect_mode`'s second tuple field, presentation.rs:66)
// and already in scope at the on_page_load call site below, so this needed
// no new plumbing, only mirroring `cutout_width_js_value`'s shape.
// `presentation_mode` (presentation.rs:14) treats `inset > 0.0` as the
// notch/hud boundary, so gating on that here keeps this function's notion
// of "a cutout was reported" identical to `Mode::Notch` itself — a hud-mode
// `inset` (always `0.0`, presentation.rs:77's fallback) renders `null`,
// exactly like `cutout_width_js_value` does for a missing cutout; App.tsx's
// HUD synthetic-height constant fills the gap client-side, same pattern as
// width.
fn cutout_height_js_value(inset: f64) -> String {
    if inset > 0.0 {
        format!("{inset}")
    } else {
        "null".into()
    }
}

// plan 087: called fresh from every tracking-area callback (mouseEntered/
// mouseMoved) — cheap (a few short-lived mutex locks, no lock held across
// the return) rather than cached, since the card's rect can change
// between events (a new item promoted, expand toggled) while the cursor
// is still resting over the window. Lock discipline: each of
// `engine.read_blocking`/the config lock acquires, reads, and drops
// before the next opens — never nested (cold-read Gap 2).
// plan 093: `cutout_height`/`idle_peek_open` added for the y-span fix —
// see `hover::active_card_rect`'s doc comment for what each means.
// `idle_peek_open` is the caller's job to supply (it needs `was_hovered`,
// which this function has no reason to know about); this function no
// longer reads `StatusState` at all — `hover::status_rail_active` (the
// old `has_status_chips` input) is gone, both the function and its call
// here, now that the y-span's idle-peek input is hover hysteresis, not
// ambient-data availability.
//
// plan 093 pushed this to 8 positional params (over clippy's default 7-arg
// threshold) by adding `cutout_height`/`idle_peek_open`. Same call as
// `Engine::new`'s own `#[allow(clippy::too_many_arguments)]` (engine.rs):
// a named-field params struct is a bigger surface change than this plan's
// scope for a function with exactly two call sites, both in this same
// file.
//
// plan 142 (v7 ticket 10 of 13, spec §6.2): `board_session_count` — when
// the Slot reads `Empty` (`!visible`), that alone can't tell this
// function whether the ambient idle surface is showing or the Agent
// Board is (the Slot has no concept of the Board at all — spec §6.1's
// precedence lives entirely in the FRONTEND's `presentationMode`). The
// caller supplies the answer via `AgentBoardPublisher::last_session_count`
// (a cheap synchronous read, not a registry round-trip) — a nonzero
// count while `!visible` means the Board is what's actually rendered
// under the cursor, so `hover::board_rect` (sized off the Board's own
// shape) is used instead of `hover::active_card_rect`'s idle formula
// (sized off the small ambient clock/weather card, which is NOT what's
// on screen in that case).
//
// Operator decision 2026-08-02 (`[agents] board_show_working`): that
// count already accounts for the Board's PRESENCE gate — the publisher
// applies it before writing the bookkeeping `last_session_count` reads
// (`agents::board::AgentBoardPublisher::gate_presence`), so a Board that
// working-only sessions were not allowed to summon reads `0` here and
// this function correctly falls back to the idle card rect. No presence
// check belongs at this call site; there is exactly one gate.
#[allow(clippy::too_many_arguments)]
#[cfg(target_os = "macos")]
fn hover_point_is_over_card(
    engine: &Engine,
    app_handle: &tauri::AppHandle,
    mode: presentation::Mode,
    cutout_width: f64,
    cutout_height: f64,
    idle_peek_open: bool,
    board_session_count: usize,
    point_x: f64,
    point_y: f64,
) -> bool {
    use crate::event::SlotState;

    let (visible, expanded) = engine.read_blocking(|q| match q.current_slot_state() {
        SlotState::Showing { expanded, .. } => (true, expanded),
        SlotState::Empty => (false, false),
    });
    let scale = app_handle
        .state::<StdMutex<Config>>()
        .lock()
        .unwrap()
        .appearance
        .card_scale;
    let rect = if !visible && board_session_count > 0 {
        hover::board_rect(
            mode,
            cutout_width,
            cutout_height,
            scale,
            board_session_count,
        )
    } else {
        hover::active_card_rect(
            mode,
            cutout_width,
            cutout_height,
            scale,
            visible,
            expanded,
            idle_peek_open,
        )
    };
    hover::point_in_rect(&rect, point_x, point_y)
}

// plan 087: the transitions-only guard — `hover-changed` must fire when
// the boolean flips and never per mouse-move (a moving cursor generates
// many mouseMoved events per second; emitting on every one would flood
// the webview and violate the idle-cost discipline plans 015/018
// established). Same emission shape as `appearance-changed`
// (`settings.rs:564`).
//
// plan 093: this is also the ONE place the TTL hover-pause hooks into
// the Engine — the same transitions-only gate that protects the webview
// from a flood of `hover-changed` events also protects the queue from a
// flood of pointless hover_enter/hover_exit calls (both are no-ops once
// already in the state they'd be set to, but there's no reason to pay a
// queue lock per mouse-move when nothing changed). `apply_blocking`
// carries the mutate→wake→emit protocol (plan 036/037) — no new side
// channel, no second wake path: this is the existing protocol, reused.
// plan 142: pushed well past clippy's default arg threshold by the five
// new board-expand parameters — same "named-field params struct is a
// bigger surface change than this ticket's scope" call as
// `hover_point_is_over_card`'s own `#[allow]` just above, and for the
// same reason (every call site is in this one file).
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
    board_expanded: &StdMutex<bool>,
) {
    use tauri::Emitter;

    {
        let mut last = was_hovered.lock().unwrap();
        if *last == hovered {
            return;
        }
        *last = hovered;
        // guard dropped at the end of this block — `apply_blocking` below
        // takes its own, unrelated queue lock; no reason to hold this
        // one across that call.
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

    // plan 142 (v7 ticket 10 of 13, spec §6.2 expanded): the Agent
    // Board's hover-expand orchestration piggybacks on this SAME
    // transitions-only gate — a hover entry over the Board (`!visible`,
    // at least one retained session) grows the real window frame and
    // opens pointer delivery; ANY transition to `hovered == false`
    // restores both immediately, whether or not this specific call is
    // the one that expanded it (`collapse_board_if_expanded` is a no-op
    // when `board_expanded` is already false).
    if hovered {
        try_expand_board_for_hover(engine, window, agent_board, board_expanded);
    } else {
        collapse_board_if_expanded(window, mode, cutout, board_expanded);
    }
}

/// plan 142: on a hover ENTRY, expand the Board's window frame + open
/// pointer delivery — but ONLY when the Slot is empty and the Board
/// actually has sessions to show (a hover entry over an ordinary
/// showing/idle card must never touch the window frame at all). Reads
/// `agent_board.last_session_count()` — the same synchronous,
/// non-registry read `hover_point_is_over_card` already uses to decide
/// which hover RECT to compare against, reused here for the same "is
/// the Board what's actually on screen" question.
#[cfg(target_os = "macos")]
fn try_expand_board_for_hover(
    engine: &Engine,
    window: &tauri::WebviewWindow,
    agent_board: &agents::board::AgentBoardPublisher,
    board_expanded: &StdMutex<bool>,
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
    // Order matters: grow the frame FIRST, then open pointer delivery —
    // never the reverse, which would briefly make the SMALL (resting)
    // frame clickable before it has grown to the rect the cursor is
    // actually over.
    if let Err(e) = window.set_size(tauri::LogicalSize::new(frame.width, frame.height)) {
        tracing::warn!("board hover-expand: set_size failed: {e}");
        return;
    }
    if let Err(e) = window.set_position(tauri::LogicalPosition::new(frame.x, frame.y)) {
        tracing::warn!("board hover-expand: set_position failed: {e}");
    }
    if let Err(e) = window.set_ignore_cursor_events(false) {
        tracing::warn!("board hover-expand: set_ignore_cursor_events(false) failed: {e}");
        return;
    }
    *board_expanded.lock().unwrap_or_else(|e| e.into_inner()) = true;
}

/// plan 142: the exit-side restore — IMMEDIATE (this runs synchronously
/// inside the same AppKit callback/shortcut handler as the transition
/// itself, never deferred), and idempotent: a hover-exit over a card
/// that never expanded anything (`*board_expanded == false` already)
/// does nothing, so this is safe to call from every `hovered == false`
/// path unconditionally.
#[cfg(target_os = "macos")]
fn collapse_board_if_expanded(
    window: &tauri::WebviewWindow,
    mode: presentation::Mode,
    cutout: Option<presentation::CutoutGeometry>,
    board_expanded: &StdMutex<bool>,
) {
    let mut expanded = board_expanded.lock().unwrap_or_else(|e| e.into_inner());
    if !*expanded {
        return;
    }
    // Reverse order from the expand path: restore click-through FIRST,
    // then shrink/reposition — never leave the enlarged frame clickable
    // for even one frame after the cursor has already left it.
    if let Err(e) = window.set_ignore_cursor_events(true) {
        tracing::warn!("board hover-collapse: set_ignore_cursor_events(true) failed: {e}");
    }
    if let Err(e) = window.set_size(tauri::LogicalSize::new(
        hover::WINDOW_WIDTH,
        hover::WINDOW_HEIGHT,
    )) {
        tracing::warn!("board hover-collapse: set_size failed: {e}");
    }
    if let Err(e) = position_window(window, mode, cutout) {
        tracing::warn!("board hover-collapse: position_window failed: {e}");
    }
    *expanded = false;
}

/// The pure half of the generic hover-latch reset (M5, see the
/// `slot-state` listener registered alongside `hover_handler` in
/// `setup`, above): extracts the visible item's `id` from a raw
/// `slot-state` wire payload (`SlotState::Showing`'s `id` field —
/// `crate::event::SLOT_STATE_EVENT`'s JSON), or `None` for `SlotState::
/// Empty` or a payload that fails to parse at all. Factored out of the
/// listener closure specifically so this parsing step is unit-testable
/// without a live window/AppKit event handler — the listener itself
/// (a mutex compare-and-swap around this call) is not.
#[cfg(target_os = "macos")]
fn visible_id_from_slot_state_payload(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()?
        .get("id")
        .and_then(|id| id.as_str())
        .map(str::to_string)
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

fn toggle_pause<R: tauri::Runtime>(engine: &Engine<R>, pause_item: &MenuItem<R>) {
    // plan 037: the mutation goes through Engine::apply_blocking (which
    // keeps the off-tokio-runtime debug_assert, wakes the rotation loop —
    // plan 015: resume/pause may change the visible item's rotation
    // deadline — and emits any slot-state change). The tray label stays
    // at the caller, driven by the closure's return value: the Engine
    // never touches menus.
    let now_paused = engine.apply_blocking(|q, now| {
        if q.is_paused() {
            q.resume();
            // v3.6 spec §4.5: resume promotes immediately, not on the next
            // rotation-loop pass
            q.tick(now);
            false
        } else {
            q.pause();
            true
        }
    });
    let _ = pause_item.set_text(if now_paused { "Resume" } else { "Pause" });
}

/// Current wall-clock instant expressed the way `silence::SilenceController`
/// needs it. The one (and only) place `chrono::Local::now()` is read for
/// silence purposes — every other silence function in this file takes an
/// `AbsoluteMinute` in, mirroring `silence.rs`'s own "the caller passes
/// time in, nothing here reads the clock" discipline.
fn now_abs_minute() -> silence::AbsoluteMinute {
    silence::absolute_minute(chrono::Local::now().naive_local())
}

/// Pure decision: does the queue's `silenced` flag need to change to match
/// the controller's verdict? `None` (the common case on most wakes/clicks —
/// nothing actually flipped) means no-op. Mirrors `toggle_pause`'s "pure
/// decision, thin apply wrapper" split, generalized to two apply wrappers
/// here (blocking for tray handlers, async for the schedule task) instead
/// of one, since this is called from both a main-thread context and a
/// tokio task.
fn silence_should_flip(queue_silenced: bool, verdict_silenced: bool) -> Option<bool> {
    (queue_silenced != verdict_silenced).then_some(verdict_silenced)
}

/// The tray's Silenced indicator text — a disabled, unclickable menu item
/// is the cheapest widget this tray idiom has for a status label (no
/// separate "status text" concept), same "reuse a MenuItem, drive it with
/// set_text" idiom `toggle_pause` already uses for Pause/Resume.
fn silence_indicator_label(silenced: bool) -> &'static str {
    if silenced {
        "Silenced"
    } else {
        "Not Silenced"
    }
}

/// The tray icon's title glyph while Silenced — spec story 14 wants the
/// state glanceable from the menu bar itself, not only inside the opened
/// menu (which is all the disabled-MenuItem indicator above can give).
/// macOS renders a tray title as text beside the icon; `None` removes it
/// entirely, so the un-Silenced menu bar looks exactly as before.
fn silence_tray_title(silenced: bool) -> Option<&'static str> {
    silenced.then_some("☾")
}

/// Pushes both Silenced indicators — the disabled menu item's text and
/// the tray icon's title glyph — to match `verdict`. The tray handle is
/// looked up by id from the menu item's own app handle so every caller
/// (tray handlers and the schedule task) stays signature-stable.
fn set_silence_indicators<R: tauri::Runtime>(indicator_item: &MenuItem<R>, verdict: bool) {
    let _ = indicator_item.set_text(silence_indicator_label(verdict));
    if let Some(tray) = indicator_item.app_handle().tray_by_id(TRAY_ID) {
        let _ = tray.set_title(silence_tray_title(verdict));
    }
}

/// The one tray icon's stable id — needed so the Silenced glyph updaters
/// can find it again after `build_tray` hands the icon to tauri.
const TRAY_ID: &str = "notchtap-tray";

/// Main-thread apply wrapper (tray handlers, off the tokio runtime — same
/// context `toggle_pause` runs in). Silences/unsilences the queue only on
/// an actual flip, logs the change, and never logs event content (this
/// path never touches an Event).
fn apply_silence_verdict_blocking<R: tauri::Runtime>(engine: &Engine<R>, verdict_silenced: bool) {
    engine.apply_blocking(|q, _now| {
        if let Some(new_state) = silence_should_flip(q.is_silenced(), verdict_silenced) {
            if new_state {
                q.silence();
            } else {
                q.unsilence();
            }
            tracing::info!(silenced = new_state, "silence state changed (tray)");
        }
    });
}

/// The async twin of `apply_silence_verdict_blocking` — the schedule task
/// (`spawn_silence_task`, below) lives on the tokio runtime, so it goes
/// through `Engine::apply` instead of `apply_blocking`.
async fn apply_silence_verdict<R: tauri::Runtime>(engine: &Engine<R>, verdict_silenced: bool) {
    engine
        .apply(|q, _now| {
            if let Some(new_state) = silence_should_flip(q.is_silenced(), verdict_silenced) {
                if new_state {
                    q.silence();
                } else {
                    q.unsilence();
                }
                tracing::info!(silenced = new_state, "silence state changed (schedule)");
            }
        })
        .await;
}

/// Recomputes the verdict from the current wall clock, applies it to the
/// queue, and refreshes the tray label — the shared tail every tray
/// mute/cancel/skip handler runs after mutating the `SilenceController`,
/// so a click takes effect immediately rather than waiting for
/// `spawn_silence_task`'s next scheduled wake.
fn refresh_silence_indicator<R: tauri::Runtime>(
    engine: &Engine<R>,
    controller: &StdMutex<silence::SilenceController>,
    indicator_item: &MenuItem<R>,
) {
    let now = now_abs_minute();
    let verdict = controller
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_silenced(now);
    apply_silence_verdict_blocking(engine, verdict);
    set_silence_indicators(indicator_item, verdict);
}

/// Shared body for the three tray mute presets — only the duration
/// differs.
fn start_mute_from_tray<R: tauri::Runtime>(
    engine: &Engine<R>,
    controller: &StdMutex<silence::SilenceController>,
    indicator_item: &MenuItem<R>,
    duration_minutes: u64,
) {
    let now = now_abs_minute();
    controller
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .start_mute(duration_minutes, now);
    refresh_silence_indicator(engine, controller, indicator_item);
}

/// plan 146a: the Silenced schedule/mute timer. Computes the verdict from
/// the CURRENT wall clock on every wake — never from a stored deadline —
/// so a clock jump (system sleep, DST, a manual date change) self-heals on
/// the very next iteration instead of needing dedicated handling; this is
/// exactly the "sleep, recompute, sleep again" contract
/// `SilenceController::next_boundary`'s own doc comment describes for its
/// conservative-wake callers. Tray mute/skip clicks (`refresh_silence_indicator`,
/// above) apply their own verdict immediately rather than waiting for this
/// loop to wake — this task only needs to catch the schedule's own
/// boundaries (window start/end) and a mute's natural expiry.
fn spawn_silence_task<R: tauri::Runtime>(
    engine: Engine<R>,
    controller: Arc<StdMutex<silence::SilenceController>>,
    indicator_item: MenuItem<R>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            let now = now_abs_minute();
            let (verdict, boundary) = {
                let c = controller.lock().unwrap_or_else(|e| e.into_inner());
                (c.is_silenced(now), c.next_boundary(now))
            };
            apply_silence_verdict(&engine, verdict).await;
            set_silence_indicators(&indicator_item, verdict);

            // `next_boundary` is conservative — it may wake this loop at a
            // boundary where the verdict doesn't actually flip (e.g. a
            // schedule window ending while a longer mute is still
            // running) — hence recomputing from scratch above rather than
            // trusting the boundary to mean "flip now". `None` (schedule
            // disabled, no mute running) falls back to an hourly
            // re-check: nothing is expected to change the verdict in that
            // state on its own, but re-evaluating from the wall clock
            // periodically rather than sleeping forever means a tray mute
            // started moments after this reaches the `None` arm is caught
            // within the hour even in the pathological case where
            // `refresh_silence_indicator`'s immediate apply somehow didn't
            // run (e.g. a future caller that mutates the controller
            // without going through the tray helpers).
            let sleep_for = match boundary {
                Some(b) => std::time::Duration::from_secs(b.saturating_sub(now).max(1) * 60),
                None => std::time::Duration::from_secs(3600),
            };
            tokio::time::sleep(sleep_for).await;
        }
    });
}

/// v6: the tray is deliberately minimal — Pause/Resume, Settings…, Quit.
/// It previously also carried "Pause Football Scores"/"Pause News" items,
/// but those duplicated the `espn_enabled`/`rss_enabled` toggles already in
/// Settings (which, since v6, also carry per-source priority and rotation
/// order — richer than a toggle belongs there, per ARCHITECTURE.md §17's
/// "everything richer than a toggle lives [in Settings], not in more tray
/// items" rule, which this tray had not yet caught up to).
///
/// plan 146a added the Silenced indicator and the mute/skip items beside
/// Pause — still rust-side only (no new invoke commands, `CLAUDE.md`'s ipc
/// & security section): every one of these mutates the session-only
/// `SilenceController` and applies the result to the queue the exact same
/// way `toggle_pause` above already does for Pause.
fn build_tray<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    engine: Engine<R>,
    start_paused: bool,
    silence_controller: Arc<StdMutex<silence::SilenceController>>,
) -> tauri::Result<(MenuItem<R>, MenuItem<R>)> {
    // v5 kill switch: a start_paused boot renders the toggle as "Resume"
    // from the first open — the label always names the *next* action.
    let initial_pause_label = if start_paused { "Resume" } else { "Pause" };
    let pause_item = MenuItem::with_id(app, "pause", initial_pause_label, true, None::<&str>)?;

    let initial_silenced = {
        let c = silence_controller.lock().unwrap_or_else(|e| e.into_inner());
        c.is_silenced(now_abs_minute())
    };
    let silenced_indicator_item = MenuItem::with_id(
        app,
        "silenced_indicator",
        silence_indicator_label(initial_silenced),
        false,
        None::<&str>,
    )?;
    let mute_30_item = MenuItem::with_id(app, "mute_30", "Mute 30 min", true, None::<&str>)?;
    let mute_60_item = MenuItem::with_id(app, "mute_60", "Mute 1 hour", true, None::<&str>)?;
    let mute_120_item = MenuItem::with_id(app, "mute_120", "Mute 2 hours", true, None::<&str>)?;
    // Always enabled: `SilenceController::cancel_mute` is already a
    // documented no-op when nothing is running, so a click while no mute
    // is active is harmless — simpler than an enabled/disabled dance kept
    // in sync with mute state across three separate handlers.
    let cancel_mute_item =
        MenuItem::with_id(app, "cancel_mute", "Cancel mute", true, None::<&str>)?;
    let skip_item = MenuItem::with_id(
        app,
        "skip_silence",
        "Skip today's silence",
        true,
        None::<&str>,
    )?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let pause_item_for_handler = pause_item.clone();
    let indicator_for_handler = silenced_indicator_item.clone();
    let controller_for_handler = silence_controller;
    let menu = Menu::new(app)?;
    menu.append(&pause_item)?;
    menu.append(&silenced_indicator_item)?;
    menu.append(&mute_30_item)?;
    menu.append(&mute_60_item)?;
    menu.append(&mute_120_item)?;
    menu.append(&cancel_mute_item)?;
    menu.append(&skip_item)?;
    menu.append(&settings_item)?;
    menu.append(&quit_item)?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "pause" => toggle_pause(&engine, &pause_item_for_handler),
            "mute_30" => {
                start_mute_from_tray(&engine, &controller_for_handler, &indicator_for_handler, 30)
            }
            "mute_60" => {
                start_mute_from_tray(&engine, &controller_for_handler, &indicator_for_handler, 60)
            }
            "mute_120" => start_mute_from_tray(
                &engine,
                &controller_for_handler,
                &indicator_for_handler,
                120,
            ),
            "cancel_mute" => {
                controller_for_handler
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .cancel_mute();
                refresh_silence_indicator(&engine, &controller_for_handler, &indicator_for_handler);
            }
            "skip_silence" => {
                let now = now_abs_minute();
                controller_for_handler
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .skip_current_window(now);
                refresh_silence_indicator(&engine, &controller_for_handler, &indicator_for_handler);
            }
            "settings" => open_settings_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    // A Silenced boot (mid-window launch) shows the glyph from the first
    // frame — the schedule task's first wake would set it anyway, but
    // that races the menu bar's first paint.
    let _ = tray.set_title(silence_tray_title(initial_silenced));

    Ok((pause_item, silenced_indicator_item))
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
    .inner_size(480.0, 700.0)
    .min_inner_size(420.0, 520.0)
    .build()
    {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => tracing::warn!("settings window failed to open: {e}"),
    }
}

// v3.6 spec §7.1.1 + plan 033: with expand-all, every promotion starts
// expanded, so the hotkey always flips — a press on an auto-expanded card
// collapses it (render-only, and disarms the auto-retract); a press on a
// collapsed card expands it and extends its rotation window 3× (manual
// expansion is the only kind that extends the turn). plan 008's High
// no-op guard is gone: there is no longer an "automatic for High" state
// to protect, since automatic expansion is now universal.
#[cfg(target_os = "macos")]
fn toggle_manual_expand<R: tauri::Runtime>(engine: &Engine<R>) {
    // plan 015: expanded changes the rotation window, so the rotation
    // loop's next deadline must be recomputed — apply_blocking wakes it.
    engine.apply_blocking(|q, _now| q.toggle_expanded());
}

#[cfg(target_os = "macos")]
fn dismiss_current<R: tauri::Runtime>(engine: &Engine<R>) {
    engine.apply_blocking(|q, now| q.dismiss_visible(now));
}

// ⌃⇧]: end the Visible item's turn as if its Rotation elapsed (Recurring
// requeues, OneShot drops) — deliberately different from ⌃⇧X's dismiss,
// which drops a Recurring item outright. See SingleSlotQueue::skip_visible.
#[cfg(target_os = "macos")]
fn skip_current<R: tauri::Runtime>(engine: &Engine<R>) {
    engine.apply_blocking(|q, now| q.skip_visible(now));
}

/// Returns the normalized (parsed re-serialized) URL iff the link is a
/// well-formed http(s) URL — the ONLY thing ⌃⇧O will hand to `open`.
/// Full parse, never a prefix check: `starts_with("http")` admits
/// `httpx://` (the same trap the settings feed validation already fixed).
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

    // -u forces URL interpretation (never a file-path fallback), and the
    // argument is the parser's own serialization — what was validated is
    // exactly what executes. The child is reaped off-thread: a dropped,
    // un-waited Child is a zombie until this 24/7 process exits.
    // absolute path (never a bare "open" resolved through $PATH): this
    // process is a 24/7 login item, so trusting the ambient PATH to
    // still resolve to the real system `open` is unnecessary risk for
    // zero benefit — /usr/bin/open is the fixed, non-configurable
    // location on every supported macOS version.
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::event::{
        test_fixtures, Event, EventSignal, EventType, Priority, RotationSpec, SlotState, SourceKind,
    };

    fn event(priority: Priority) -> Event {
        test_fixtures::with_priority(test_fixtures::event("t"), priority)
    }

    // ---- plan 146a: silence_should_flip / silence_indicator_label ----

    #[test]
    fn silence_should_flip_is_none_when_already_matching() {
        assert_eq!(silence_should_flip(false, false), None);
        assert_eq!(silence_should_flip(true, true), None);
    }

    #[test]
    fn silence_should_flip_reports_the_new_state_on_a_mismatch() {
        assert_eq!(silence_should_flip(false, true), Some(true));
        assert_eq!(silence_should_flip(true, false), Some(false));
    }

    #[test]
    fn silence_indicator_label_names_the_current_state() {
        assert_eq!(silence_indicator_label(true), "Silenced");
        assert_eq!(silence_indicator_label(false), "Not Silenced");
    }

    #[test]
    fn silence_tray_title_shows_a_glyph_only_while_silenced() {
        assert_eq!(silence_tray_title(true), Some("☾"));
        assert_eq!(silence_tray_title(false), None);
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
        // presentation.rs's hud fallback (`detect_mode`'s Err arm) reports
        // exactly this: `inset: 0.0`.
        assert_eq!(cutout_height_js_value(0.0), "null");
    }

    #[test]
    fn cutout_height_js_value_renders_null_for_a_negative_inset() {
        // defensive: never observed from the shim, but the `> 0.0` guard
        // (matching `presentation_mode`'s own boundary) must not render a
        // negative number as if it were a real height.
        assert_eq!(cutout_height_js_value(-1.0), "null");
    }

    fn test_engine(app: &tauri::App<tauri::test::MockRuntime>) -> Engine<tauri::test::MockRuntime> {
        Engine::new(
            SingleSlotQueue::new(50),
            app.handle().clone(),
            Arc::new(Vec::new()),
            false,
            false,
            false,
            false,
            None,
        )
    }

    #[test]
    fn toggle_manual_expand_collapses_an_auto_expanded_high_item() {
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        engine.apply_blocking(|q, now| q.enqueue(event(Priority::High), now).unwrap());

        // every promotion auto-expands (plan 033) — confirm that baseline
        // first, then prove the hotkey flips it: plan 008's High no-op
        // guard is deleted, so the press must collapse the card.
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

        // Medium auto-expands on promotion too (plan 033): the first press
        // collapses, the second re-expands.
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
        // consume the queue's initial Empty baseline (see the dismiss no-op
        // assertion below) so the post-call state proves the handler
        // changed nothing
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
            // the skipped Recurring item survived — this is what distinguishes
            // skip_current from dismiss_current (whose test proves the drop)
            assert_eq!(q.total_waiting(), 1);
            // and it comes back: skip the next item too and the recurring one
            // promotes again
            q.skip_visible(now);
            match q.current_slot_state() {
                SlotState::Showing { id, .. } => assert_eq!(id, recurring_id),
                SlotState::Empty => panic!("expected recurring item to return"),
            }
        });
    }

    #[test]
    fn toggle_pause_updates_label_and_promotes_on_resume() {
        let app = tauri::test::mock_app();
        let pause_item =
            MenuItem::with_id(app.handle(), "pause", "Pause", true, None::<&str>).unwrap();
        let engine = test_engine(&app);

        toggle_pause(&engine, &pause_item);
        assert_eq!(pause_item.text().unwrap(), "Resume");
        engine.apply_blocking(|q, now| {
            assert!(q.is_paused());
            q.enqueue(event(Priority::Medium), now).unwrap();
            assert_eq!(q.current_slot_state(), SlotState::Empty);
        });

        toggle_pause(&engine, &pause_item);
        assert_eq!(pause_item.text().unwrap(), "Pause");
        engine.read_blocking(|q| {
            assert!(!q.is_paused());
            assert!(matches!(q.current_slot_state(), SlotState::Showing { .. }));
            assert_eq!(q.total_waiting(), 0);
        });
    }

    #[test]
    fn openable_http_url_accepts_only_normalized_http_urls() {
        // Accepting cases: the returned string is always the parser's own
        // normalized serialization, never the raw input (the tab/newline
        // cases prove that — WHATWG strips those before serializing).
        for raw in [
            "https://example.com/a",
            "http://example.com",
            "  https://example.com  ",
            "https://exa\tmple.com/pa\nth",
        ] {
            let expected = reqwest::Url::parse(raw).unwrap().to_string();
            assert_eq!(
                openable_http_url(raw),
                Some(expected),
                "should accept and normalize: {raw:?}"
            );
        }

        // Rejecting cases: non-http(s) schemes and unparseable input.
        // `httpx://` is the prefix trap `starts_with(\"http\")` would admit.
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
        // config-load error text can carry a quoted TOML key or a windows-
        // style path segment (backslash) verbatim — both must be escaped
        // or they'd break out of the AppleScript string literal
        // `show_boot_error_dialog` splices this into.
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
        // Empty slot → current_link() is None → early return before any
        // spawn. Proves the guard; the `open` subprocess stays unreached.
        let app = tauri::test::mock_app();
        let engine = test_engine(&app);
        // consume the queue's initial Empty baseline (see the dismiss no-op
        // test) so the post-call assertion proves the handler changed nothing
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
        // all three hazards in the title: `</script>`, U+2028, U+2029.
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

        // the escapes are valid JSON escapes, so the output is safe for JS
        // AND still the same data: it parses, and the value is unchanged.
        let parsed: serde_json::Value =
            serde_json::from_str(&escaped).expect("escaped output must still parse as JSON");
        assert_eq!(parsed["title"].as_str().unwrap(), title);
    }
}
