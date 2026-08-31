//! The Icon Strip's click path. A native `NSEvent` local monitor
//! observes the `mouseDown` rust-side, resolves which icon it landed on,
//! updates the one shared Tab Selection, and emits
//! `tab-selection-changed` — the same rust-decides/frontend-renders
//! shape as `hover-changed`. A webview `onClick` cannot do this job: the
//! overlay is receive-only (`capabilities/default.json` grants event
//! listen/unlisten and nothing else — no invoke, no emit), so a click
//! the webview sees has no channel back to the rust side that owns the
//! selection. Local (app-scoped) monitors only see events the window
//! server actually dispatches to this app, which requires
//! `set_ignore_cursor_events(false)` — `lib.rs`'s hover transition
//! handler sets that exactly while the strip is the live hover target.
//! [`click_target`] is the pure, unit-tested decision; installing the
//! monitor is the impure shell (`docs/TESTING_STRATEGY.md` §4.4).

use crate::hover::{point_in_rect, Rect};
use crate::tabs::Tab;

/// Which tab (if any) a click at window-coordinate `(x, y)` selects.
/// `present` and `rects` MUST be the same length and index-aligned —
/// `hover::icon_strip_rects` lays out one rect per PRESENT icon in
/// `Tab::ORDER` order, and `tabs::present_tabs` produces the present
/// list in that same order, so zipping is the whole contract.
pub fn click_target(x: f64, y: f64, present: &[Tab], rects: &[Rect]) -> Option<Tab> {
    rects
        .iter()
        .zip(present)
        .find(|(rect, _)| point_in_rect(rect, x, y))
        .map(|(_, tab)| *tab)
}

#[cfg(target_os = "macos")]
pub struct ClickMonitorParams<R: tauri::Runtime> {
    pub app: tauri::AppHandle<R>,
    pub tab_wire: std::sync::Arc<crate::tabs::TabWire>,
    pub was_hovered: std::sync::Arc<std::sync::Mutex<bool>>,
    /// The overlay panel's `windowNumber` — the monitor sees every event
    /// dispatched to this app and must act only on ours.
    pub window_number: isize,
    pub mode: crate::presentation::Mode,
    pub cutout_width: f64,
    pub cutout_height: f64,
    /// The REAL currently-applied window height, read fresh per click —
    /// never `hover::WINDOW_HEIGHT`. The Agent Board's hover-expand
    /// genuinely resizes this window taller while the Slot is idle,
    /// which is exactly when the strip is up, so a resting-constant
    /// y-transform would put every icon rect in the wrong place. Same
    /// parameter, same reason, as `hover::board_rect`'s and
    /// `icon_strip_rects`' own `window_height`.
    pub board_frame: std::sync::Arc<std::sync::Mutex<crate::BoardFrameState>>,
}

/// Installs the `LeftMouseDown` local monitor. Main-thread only (AppKit
/// event monitors are); call once from `setup`. The monitor lives for
/// the app's whole lifetime — the returned token is deliberately
/// forgotten, matching the tracking area's own install-once posture.
///
/// The handler NEVER swallows the event (always returns it unchanged):
/// selection is a side effect, and the webview under the strip still
/// gets its own mousedown for `:active` press feedback.
#[cfg(target_os = "macos")]
pub fn install_click_monitor<R: tauri::Runtime>(params: ClickMonitorParams<R>) {
    use block2::RcBlock;
    use objc2_app_kit::{NSEvent, NSEventMask};
    use tauri::Manager;

    let ClickMonitorParams {
        app,
        tab_wire,
        was_hovered,
        window_number,
        mode,
        cutout_width,
        cutout_height,
        board_frame,
    } = params;

    let handler = RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
        let pass_through = event.as_ptr();
        let event_ref = unsafe { event.as_ref() };
        if event_ref.windowNumber() != window_number {
            return pass_through;
        }
        // The strip is on screen exactly when the shell is hovered
        // AND no pushed card occupies the Slot — the same two gates
        // the frontend renders it under. Clicks in any other state
        // pass through untouched.
        if !*was_hovered.lock().unwrap_or_else(|e| e.into_inner()) {
            return pass_through;
        }
        if tab_wire
            .slot_occupied
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return pass_through;
        }
        let loc = event_ref.locationInWindow();
        let present = tab_wire
            .tabs
            .presence
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let real_window_height = board_frame.lock().unwrap_or_else(|e| e.into_inner()).height;
        // Live, never the boot value: `set_appearance` hot-applies a new
        // `card_scale` at runtime, and the hover hit-test reads it per
        // event for exactly this reason (`hover_point_is_over_card`).
        let scale = app
            .state::<std::sync::Mutex<crate::config::Config>>()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .appearance
            .card_scale;
        let rects = crate::hover::icon_strip_rects(
            mode,
            cutout_width,
            cutout_height,
            scale,
            present.len(),
            real_window_height,
        );
        if let Some(tab) = click_target(loc.x, loc.y, &present, &rects) {
            tracing::debug!(?tab, "icon strip click");
            // The ONE shared mutation path — identical semantics for a
            // click and a prefix+digit, including the news-visit charge
            // clear and the transitions-only emit.
            crate::apply_tab_select(&app, &tab_wire, tab);
        }
        pass_through
    });

    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseDown, &handler)
    };
    // Install-once, app-lifetime: intentionally never removed.
    std::mem::forget(monitor);
    std::mem::forget(handler);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x_min: f64, x_max: f64) -> Rect {
        Rect {
            x_min,
            x_max,
            y_min: 268.0,
            y_max: 300.0,
        }
    }

    #[test]
    fn click_inside_an_icon_box_selects_that_tab() {
        let present = vec![Tab::Football, Tab::News];
        let rects = vec![rect(400.0, 418.0), rect(426.0, 444.0)];
        assert_eq!(
            click_target(430.0, 280.0, &present, &rects),
            Some(Tab::News)
        );
    }

    #[test]
    fn click_in_the_gap_between_icons_selects_nothing() {
        let present = vec![Tab::Football, Tab::News];
        let rects = vec![rect(400.0, 418.0), rect(426.0, 444.0)];
        assert_eq!(click_target(420.0, 280.0, &present, &rects), None);
    }

    #[test]
    fn click_outside_the_strip_y_band_selects_nothing() {
        let present = vec![Tab::News];
        let rects = vec![rect(400.0, 418.0)];
        assert_eq!(click_target(410.0, 100.0, &present, &rects), None);
    }

    #[test]
    fn present_list_and_rects_zip_index_for_index() {
        // Three present icons — the middle one must map to the middle
        // rect, not to its Tab::ORDER position among all five.
        let present = vec![Tab::Agent, Tab::News, Tab::News];
        let rects = vec![rect(374.0, 392.0), rect(400.0, 418.0), rect(426.0, 444.0)];
        assert_eq!(
            click_target(410.0, 280.0, &present, &rects),
            Some(Tab::News)
        );
    }
}
