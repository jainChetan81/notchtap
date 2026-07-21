//! Plan 087: the hover primitive's pure geometry/state logic — no AppKit
//! types anywhere in this module. `lib.rs` wires the `tauri-nspanel`
//! tracking-area callbacks to the functions here; this module never
//! touches a window, a lock, or an event object, so it is unit-testable
//! without a GUI, the same discipline `presentation::presentation_mode`
//! follows (`docs/TESTING_STRATEGY.md` §4.4).
//!
//! Rationale of record: `docs/design/hover-cursor-tracking.md` (the
//! spike this module implements) — §2 for why a tracking area works at
//! all under `set_ignore_cursor_events(true)`, §6 for the rect-derivation
//! decision and the rejected `report_card_bounds` alternative, §7 for the
//! recommendation.

use crate::presentation::Mode;
use crate::status::StatusState;

/// The fixed overlay window's size — `src-tauri/tauri.conf.json`'s
/// `"width": 500, "height": 300`, `"resizable": false`. The window frame
/// never changes; only the CSS width *within* it does (see the width
/// table below). Duplicated here as named constants (not read from the
/// conf file at runtime) because this module takes no I/O — it is pure
/// numbers in, pure `Rect` out.
const WINDOW_WIDTH: f64 = 500.0;
const WINDOW_HEIGHT: f64 = 300.0;

// Card width breakpoints — duplicated-constants pair with
// `src/styles.css:25-61`. Any future change to a card width there MUST
// change these too (see the `active_card_rect` doc comment and the
// named-constant test below, which is the tripwire).
const BASE_WIDTH: f64 = 400.0; // .rail-card, styles.css:25
const EXPANDED_WIDTH: f64 = 500.0; // .rail-card.expanded, styles.css:39
const IDLE_WIDTH: f64 = 270.0; // .rail-card.idle, styles.css:44
const IDLE_STATUS_WIDTH: f64 = 460.0; // .rail-card.idle.status, styles.css:52
                                      // notch-mode clamp bounds — same 270/460 pair, styles.css:61.
const NOTCH_CLAMP_MIN: f64 = IDLE_WIDTH;
const NOTCH_CLAMP_MAX: f64 = IDLE_STATUS_WIDTH;

/// A screen-space rect in AppKit window coordinates (bottom-left origin,
/// y grows UP) — the region where hover should count as "over the
/// card." Plain numbers, no AppKit types, so `point_in_rect` and every
/// test below need no GUI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

/// Inclusive-bounds point-in-rect test — a point exactly on an edge
/// counts as inside (matches the CONSERVATIVE philosophy: never
/// narrower than the true rendered edge).
pub fn point_in_rect(rect: &Rect, x: f64, y: f64) -> bool {
    x >= rect.x_min && x <= rect.x_max && y >= rect.y_min && y <= rect.y_max
}

/// Cold-read Gap 3: the coordinate-space flip. `locationInWindow`
/// (AppKit) is bottom-left origin, y grows UP; the CSS card is laid out
/// top-down in a window pinned flush to the physical screen top (`y`
/// stays `0.0`, `position_window`). Getting this backwards is silent — a
/// y-comparison never panics, it just stops matching the rendered card.
///
/// Formula: a top-down rect at CSS y-offset `top` with height `height`,
/// inside a window of height `window_height`, occupies AppKit y from
/// `window_height - top - height` (low/bottom) to `window_height - top`
/// (high/top). A rect at the very top of the window (`top == 0.0`) maps
/// to the window's HIGHEST AppKit y, not its lowest — that is the
/// specific inversion this helper exists to get right, once, in one
/// place.
pub fn css_top_down_to_appkit_y(window_height: f64, top: f64, height: f64) -> (f64, f64) {
    let high = window_height - top;
    let low = high - height;
    (low, high)
}

/// Cold-read Gap 1: `has_status_chips` is not rust-owned today — the
/// predicate lives only in TypeScript (`src/useStatusState.ts:106`,
/// `statusRailActive`). This is the rust mirror; the two copies carry a
/// cross-reference comment naming the other. Keep the seven terms and
/// their order identical to the TS original — the two easiest to miss
/// are `waiting > 0` and `paused`, which have nothing to do with a
/// source's `enabled` gate.
///
/// Mirror of `src/useStatusState.ts:106`'s `statusRailActive`. Any
/// change to either predicate's terms must change both.
pub fn status_rail_active(status: &StatusState) -> bool {
    status.football.enabled
        || status.news.enabled
        || status.football.live.is_some()
        || status.weather.enabled
        || status.weather.current.is_some()
        || status.waiting > 0
        || status.paused
}

/// The screen-space rect, in AppKit window coordinates, currently
/// covered by the rendered card — the region where hover should count.
/// Deliberately CONSERVATIVE: it may be slightly wider than the true
/// rendered edge (the spike's §6 decision), never narrower.
///
/// Mirrors the width breakpoints in `src/styles.css:25-61`. Any change
/// to a card width there MUST change the constants at the top of this
/// file — see `active_card_rect_widths_match_named_style_constants`.
///
/// The vertical span is deliberately the FULL window height (no partial
/// `top`/`height` narrowing): unlike width, `styles.css` has no per-state
/// height breakpoints to mirror, and the CONSERVATIVE philosophy (never
/// narrower than truth) permits skipping a height guess entirely by
/// covering the whole fixed 300px window. `css_top_down_to_appkit_y` is
/// still used for that span (not hardcoded `0.0..WINDOW_HEIGHT` inline)
/// so the one coordinate-flip seam stays in exactly one place.
///
/// `scale` is `Config.appearance.card_scale` (user-configurable via the
/// Settings Appearance section, default `1.0`) — a COSMETIC preference.
///
/// Plan 090 (Q1a): `scale` must NOT be applied to the `Mode::Notch`
/// width. That width mirrors `--notchtap-cutout-width`
/// (`src/styles.css:61`), which is itself a hardware measurement — the
/// physical `NSScreen` safe-area inset, read via the `notchtap-detect`
/// subprocess — not a design width. The physical notch does not get
/// wider because the user picked a bigger card, so scaling it would
/// silently reintroduce the menu-bar-overlap defect the plan fixes (see
/// `plans/090-card-scale-vs-hardware-geometry.md` for the full
/// rationale and the operator's Decision). Every OTHER width (every
/// `Mode::Hud` arm) IS a cosmetic design width and IS multiplied by
/// `scale` in `styles.css` via `var(--card-scale)`, so those arms must
/// keep doing the same here or the rect silently drifts from the
/// rendered card for any user not at scale 1.0. Do not "fix" the
/// `Mode::Notch` arm back to multiplying by `scale` — that reverts a
/// deliberate, decided exemption, not an oversight.
pub fn active_card_rect(
    mode: Mode,
    cutout_width: f64,
    scale: f64,
    visible: bool,
    expanded: bool,
    has_status_chips: bool,
) -> Rect {
    let width = match mode {
        Mode::Notch => cutout_width.clamp(NOTCH_CLAMP_MIN, NOTCH_CLAMP_MAX),
        Mode::Hud if visible && expanded => EXPANDED_WIDTH * scale,
        Mode::Hud if visible => BASE_WIDTH * scale,
        Mode::Hud if has_status_chips => IDLE_STATUS_WIDTH * scale,
        Mode::Hud => IDLE_WIDTH * scale,
    };

    let x_min = (WINDOW_WIDTH - width) / 2.0;
    let (y_min, y_max) = css_top_down_to_appkit_y(WINDOW_HEIGHT, 0.0, WINDOW_HEIGHT);

    Rect {
        x_min,
        x_max: x_min + width,
        y_min,
        y_max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::{FootballStatus, LiveMatchSummary, NewsStatus, WeatherStatus};

    fn status(
        paused: bool,
        waiting: usize,
        football_enabled: bool,
        football_live: bool,
        news_enabled: bool,
        weather_enabled: bool,
        weather_current: bool,
    ) -> StatusState {
        StatusState {
            paused,
            waiting,
            football: FootballStatus {
                enabled: football_enabled,
                live: football_live.then(|| LiveMatchSummary {
                    label: "Home 1-0 Away".into(),
                    minute: "45'".into(),
                }),
            },
            news: NewsStatus {
                enabled: news_enabled,
            },
            weather: WeatherStatus {
                enabled: weather_enabled,
                current: weather_current.then(|| crate::status::WeatherSummary {
                    temp_display: "27°".into(),
                    condition: "Cloudy".into(),
                }),
            },
        }
    }

    fn all_false() -> StatusState {
        status(false, 0, false, false, false, false, false)
    }

    // Cold-read Gap 1: one case per term of the seven-term predicate —
    // each alone makes the whole thing true; all false makes it false.
    #[test]
    fn status_rail_active_all_false_is_false() {
        assert!(!status_rail_active(&all_false()));
    }

    #[test]
    fn status_rail_active_football_enabled_alone_is_true() {
        assert!(status_rail_active(&status(
            false, 0, true, false, false, false, false
        )));
    }

    #[test]
    fn status_rail_active_news_enabled_alone_is_true() {
        assert!(status_rail_active(&status(
            false, 0, false, false, true, false, false
        )));
    }

    #[test]
    fn status_rail_active_football_live_alone_is_true() {
        assert!(status_rail_active(&status(
            false, 0, false, true, false, false, false
        )));
    }

    #[test]
    fn status_rail_active_weather_enabled_alone_is_true() {
        assert!(status_rail_active(&status(
            false, 0, false, false, false, true, false
        )));
    }

    #[test]
    fn status_rail_active_weather_current_alone_is_true() {
        assert!(status_rail_active(&status(
            false, 0, false, false, false, false, true
        )));
    }

    // The two easiest to miss per the plan's own callout.
    #[test]
    fn status_rail_active_waiting_gt_zero_alone_is_true() {
        assert!(status_rail_active(&status(
            false, 1, false, false, false, false, false
        )));
    }

    #[test]
    fn status_rail_active_paused_alone_is_true() {
        assert!(status_rail_active(&status(
            true, 0, false, false, false, false, false
        )));
    }

    // --- active_card_rect: every mode/state branch, at scale 1.0 ---

    #[test]
    fn hud_visible_expanded_is_500_at_scale_1() {
        let r = active_card_rect(Mode::Hud, 0.0, 1.0, true, true, false);
        assert_eq!(r.x_max - r.x_min, EXPANDED_WIDTH);
    }

    #[test]
    fn hud_visible_not_expanded_is_400_at_scale_1() {
        let r = active_card_rect(Mode::Hud, 0.0, 1.0, true, false, false);
        assert_eq!(r.x_max - r.x_min, BASE_WIDTH);
    }

    #[test]
    fn hud_idle_with_status_chips_is_460_at_scale_1() {
        let r = active_card_rect(Mode::Hud, 0.0, 1.0, false, false, true);
        assert_eq!(r.x_max - r.x_min, IDLE_STATUS_WIDTH);
    }

    #[test]
    fn hud_idle_without_status_chips_is_270_at_scale_1() {
        let r = active_card_rect(Mode::Hud, 0.0, 1.0, false, false, false);
        assert_eq!(r.x_max - r.x_min, IDLE_WIDTH);
    }

    // --- the same branches at scale 0.8 and 1.25 (the pinned gap) ---

    #[test]
    fn hud_visible_expanded_scales_at_0_8() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.8, true, true, false);
        assert_eq!(r.x_max - r.x_min, EXPANDED_WIDTH * 0.8);
    }

    #[test]
    fn hud_visible_expanded_scales_at_1_25() {
        let r = active_card_rect(Mode::Hud, 0.0, 1.25, true, true, false);
        assert_eq!(r.x_max - r.x_min, EXPANDED_WIDTH * 1.25);
    }

    #[test]
    fn hud_visible_not_expanded_scales_at_0_8() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.8, true, false, false);
        assert_eq!(r.x_max - r.x_min, BASE_WIDTH * 0.8);
    }

    #[test]
    fn hud_visible_not_expanded_scales_at_1_25() {
        let r = active_card_rect(Mode::Hud, 0.0, 1.25, true, false, false);
        assert_eq!(r.x_max - r.x_min, BASE_WIDTH * 1.25);
    }

    #[test]
    fn hud_idle_with_status_chips_scales_at_0_8() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.8, false, false, true);
        assert_eq!(r.x_max - r.x_min, IDLE_STATUS_WIDTH * 0.8);
    }

    #[test]
    fn hud_idle_with_status_chips_scales_at_1_25() {
        let r = active_card_rect(Mode::Hud, 0.0, 1.25, false, false, true);
        assert_eq!(r.x_max - r.x_min, IDLE_STATUS_WIDTH * 1.25);
    }

    #[test]
    fn hud_idle_without_status_chips_scales_at_0_8() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.8, false, false, false);
        assert_eq!(r.x_max - r.x_min, IDLE_WIDTH * 0.8);
    }

    #[test]
    fn hud_idle_without_status_chips_scales_at_1_25() {
        let r = active_card_rect(Mode::Hud, 0.0, 1.25, false, false, false);
        assert_eq!(r.x_max - r.x_min, IDLE_WIDTH * 1.25);
    }

    // --- notch-mode clamp at both bounds — plan 090 (Q1a): `scale` is
    // NOT applied to notch-mode width, because it mirrors a hardware
    // measurement (the physical cutout), not a cosmetic design width.
    // These pin the clamp bounds at several scales to confirm the
    // result is identical regardless of `scale` — renamed from
    // `_at_scale_N` (which implied scale-dependence) to `_ignores_scale`
    // where the old name would now mislead.

    #[test]
    fn notch_clamp_floors_narrow_cutout_ignores_scale_at_1_0() {
        let r = active_card_rect(Mode::Notch, 200.0, 1.0, false, false, false);
        assert_eq!(r.x_max - r.x_min, NOTCH_CLAMP_MIN);
    }

    #[test]
    fn notch_clamp_ceils_wide_cutout_at_scale_1() {
        let r = active_card_rect(Mode::Notch, 600.0, 1.0, false, false, false);
        assert_eq!(r.x_max - r.x_min, NOTCH_CLAMP_MAX);
    }

    #[test]
    fn notch_clamp_floors_narrow_cutout_ignores_scale_at_0_8() {
        let r = active_card_rect(Mode::Notch, 200.0, 0.8, false, false, false);
        assert_eq!(r.x_max - r.x_min, NOTCH_CLAMP_MIN);
    }

    #[test]
    fn notch_clamp_ceils_wide_cutout_ignores_scale_at_1_25() {
        let r = active_card_rect(Mode::Notch, 600.0, 1.25, false, false, false);
        assert_eq!(r.x_max - r.x_min, NOTCH_CLAMP_MAX);
    }

    // New invariant pinned directly (not just via clamp bounds): at a
    // realistic cutout width (319px — plan 063's own fixture,
    // `src-tauri/src/lib.rs:983`), the WHOLE rect — not just its width —
    // must be byte-for-byte identical at scale 1.0 vs 1.25, since
    // `x_min`/`x_max` both derive from the (now-unscaled) notch width.
    #[test]
    fn notch_mode_rect_is_identical_across_scale_for_realistic_cutout() {
        let at_scale_1 = active_card_rect(Mode::Notch, 319.0, 1.0, false, false, false);
        let at_scale_1_25 = active_card_rect(Mode::Notch, 319.0, 1.25, false, false, false);
        assert_eq!(at_scale_1, at_scale_1_25);
    }

    // --- point_in_rect: just inside vs just outside each edge ---

    #[test]
    fn point_in_rect_just_inside_and_outside_each_edge() {
        let rect = Rect {
            x_min: 50.0,
            x_max: 150.0,
            y_min: 20.0,
            y_max: 80.0,
        };
        assert!(
            point_in_rect(&rect, 50.0, 50.0),
            "on the left edge counts as inside"
        );
        assert!(
            !point_in_rect(&rect, 49.999, 50.0),
            "just left of the left edge is outside"
        );
        assert!(
            point_in_rect(&rect, 150.0, 50.0),
            "on the right edge counts as inside"
        );
        assert!(
            !point_in_rect(&rect, 150.001, 50.0),
            "just right of the right edge is outside"
        );
        assert!(
            point_in_rect(&rect, 100.0, 20.0),
            "on the bottom edge counts as inside"
        );
        assert!(
            !point_in_rect(&rect, 100.0, 19.999),
            "just below the bottom edge is outside"
        );
        assert!(
            point_in_rect(&rect, 100.0, 80.0),
            "on the top edge counts as inside"
        );
        assert!(
            !point_in_rect(&rect, 100.0, 80.001),
            "just above the top edge is outside"
        );
    }

    // --- the named-constant assertion (duplicated-constants tripwire) ---

    // Mirrors src/styles.css:25 (.rail-card), :39 (.expanded), :44 (.idle),
    // :52 (.idle.status), :61 (the notch-mode clamp) — a NAMED-constant
    // assertion, not a live CSS parse (spike §6's explicit simplification).
    // If a future edit changes a width in styles.css without updating the
    // constants at the top of this file, this test does NOT catch it by
    // itself (it only asserts internal self-consistency) — it exists so a
    // reviewer diffing this file sees the citations and checks both sides.
    #[test]
    fn active_card_rect_widths_match_named_style_constants() {
        assert_eq!(BASE_WIDTH, 400.0);
        assert_eq!(EXPANDED_WIDTH, 500.0);
        assert_eq!(IDLE_WIDTH, 270.0);
        assert_eq!(IDLE_STATUS_WIDTH, 460.0);
        assert_eq!(NOTCH_CLAMP_MIN, 270.0);
        assert_eq!(NOTCH_CLAMP_MAX, 460.0);
    }

    // --- cold-read Gap 3: the coordinate-space flip, unit-tested on its own ---

    #[test]
    fn top_of_window_maps_to_high_appkit_y_not_low() {
        // A 50px-tall rect at the very top of a 300px window (top == 0.0)
        // must occupy the HIGH end of AppKit's y-range (250..300), not the
        // low end — the specific inversion Gap 3 flagged.
        let (low, high) = css_top_down_to_appkit_y(300.0, 0.0, 50.0);
        assert_eq!(high, 300.0);
        assert_eq!(low, 250.0);
        assert!(
            low > 0.0 && high > low,
            "the top-of-window rect is nowhere near AppKit y=0"
        );
    }

    #[test]
    fn bottom_of_window_maps_to_low_appkit_y() {
        // The mirror case: a rect flush against the CSS bottom (top ==
        // window_height - height) must land at the LOW end of AppKit's
        // y-range, confirming the flip works in both directions.
        let (low, high) = css_top_down_to_appkit_y(300.0, 250.0, 50.0);
        assert_eq!(low, 0.0);
        assert_eq!(high, 50.0);
    }

    #[test]
    fn full_window_height_span_is_the_whole_appkit_range() {
        let (low, high) = css_top_down_to_appkit_y(WINDOW_HEIGHT, 0.0, WINDOW_HEIGHT);
        assert_eq!(low, 0.0);
        assert_eq!(high, WINDOW_HEIGHT);
    }

    #[test]
    fn active_card_rect_y_span_is_the_full_window_height() {
        // Documents the deliberate "no height breakpoints to mirror"
        // choice: the rect's y-range always covers the whole fixed window.
        let r = active_card_rect(Mode::Hud, 0.0, 1.0, false, false, false);
        assert_eq!(r.y_min, 0.0);
        assert_eq!(r.y_max, WINDOW_HEIGHT);
    }
}
