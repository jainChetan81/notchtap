//! Menu-bar tray and the Silenced schedule/mute timer.
//!
//! Owns the tray menu items, the Pause toggle, the Silenced indicators, and
//! the task that keeps the queue's Silenced flag matching the
//! `SilenceController`'s verdict. Every item here is rust-side only: the tray
//! never adds an invoke command, it mutates the queue through the Engine.

use std::sync::{Arc, Mutex as StdMutex};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;

use crate::engine::Engine;
use crate::silence;

pub(crate) fn toggle_pause<R: tauri::Runtime>(engine: &Engine<R>, pause_item: &MenuItem<R>) {
    // the mutation goes through Engine::apply_blocking (which
    // keeps the off-tokio-runtime debug_assert, wakes the rotation loop —
    // resume/pause may change the visible item's rotation
    // deadline — and emits any slot-state change). The tray label stays
    // at the caller, driven by the closure's return value: the Engine
    // never touches menus.
    let now_paused = engine.apply_blocking(|q, now| {
        if q.is_paused() {
            q.resume();
            // resume promotes immediately, not on the next rotation-loop
            // pass
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

/// The tray icon's title glyph while Silenced — the state must be
/// glanceable from the menu bar itself, not only inside the opened menu
/// (which is all the disabled-MenuItem indicator above can give). macOS
/// renders a tray title as text beside the icon; `None` removes it
/// entirely, so the un-Silenced menu bar carries no extra text.
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

/// the Silenced schedule/mute timer. Computes the verdict from
/// the CURRENT wall clock on every wake — never from a stored deadline —
/// so a clock jump (system sleep, DST, a manual date change) self-heals on
/// the very next iteration instead of needing dedicated handling; this is
/// exactly the "sleep, recompute, sleep again" contract
/// `SilenceController::next_boundary`'s own doc comment describes for its
/// conservative-wake callers. Tray mute/skip clicks (`refresh_silence_indicator`,
/// above) apply their own verdict immediately rather than waiting for this
/// loop to wake — this task only needs to catch the schedule's own
/// boundaries (window start/end) and a mute's natural expiry.
pub(crate) fn spawn_silence_task<R: tauri::Runtime>(
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

/// The tray is deliberately minimal — Pause/Resume, the Silenced indicator
/// and mute/skip items, Settings…, Quit. Anything richer than a toggle
/// belongs in Settings, not in more tray items (`docs/ARCHITECTURE.md` §17).
///
/// Every item here is rust-side only — no new invoke commands, per
/// `CLAUDE.md`'s ipc & security section: each mutates the session-only
/// `SilenceController` and applies the result to the queue exactly the way
/// `toggle_pause` above does for Pause.
pub(crate) fn build_tray<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    engine: Engine<R>,
    start_paused: bool,
    silence_controller: Arc<StdMutex<silence::SilenceController>>,
) -> tauri::Result<(MenuItem<R>, MenuItem<R>)> {
    // Kill Switch: a start_paused boot renders the toggle as "Resume" from
    // the first open — the label always names the *next* action.
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

/// Lazy creation, focus-if-open. A normal decorated window —
/// everything the overlay is not (no nspanel, no always-on-top, no
/// collection-behavior calls); closing it leaves the app running.
pub(crate) fn open_settings_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::event::{test_fixtures, Event, Priority, SlotState};
    use crate::queue::SingleSlotQueue;

    fn event(priority: Priority) -> Event {
        test_fixtures::with_priority(test_fixtures::event("t"), priority)
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
}
