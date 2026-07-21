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

// Geometry-contract constants — duplicated-constants pair with
// `src/styles.css`'s `.card-assembly`/`.card-assembly.idle`/
// `.card-assembly.expanded` rules and App.tsx's HUD synthetic constants.
// Any future change to one of these numbers anywhere MUST change every
// other copy in the same commit (see the `active_card_rect` doc comment
// and the named-constant test below, which is the tripwire).
// plan 091: replaces the old BASE_WIDTH/EXPANDED_WIDTH/IDLE_WIDTH/
// IDLE_STATUS_WIDTH/NOTCH_CLAMP_MIN/NOTCH_CLAMP_MAX set — the idle/idle-
// status width split (plan 034's 270/460 distinction) deliberately
// collapses here too: the new idle has ONE width formula regardless of
// status chips, because the status dots replace the chip rail entirely.
const FLANK_IDLE: f64 = 85.0; // idle flank width, styles.css .card-assembly.idle
const MIN_FLANK_SHOWING: f64 = 60.0; // showing/expanded minimum flank width
const BASE_SHOWING: f64 = 400.0; // .card-assembly (showing) design-width floor
const BASE_EXPANDED: f64 = 500.0; // .card-assembly.expanded design-width floor
const HUD_CUTOUT_W: f64 = 200.0; // App.tsx's HUD synthetic cutout width
                                 // App.tsx's HUD synthetic cutout height — not consumed by
                                 // `active_card_rect`'s own math (the rect's y-span stays the full window
                                 // height, that function's doc comment). Kept as a named constant purely
                                 // so `active_card_rect_geometry_constants_match_named_style_constants`
                                 // pins it alongside every other Geometry-contract number — a reviewer
                                 // diffing styles.css's synthetic-cutout height then sees both sides.
                                 // `#[allow(dead_code)]`: real, but its only reader is that test.
#[allow(dead_code)]
const HUD_CUTOUT_H: f64 = 32.0;

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
/// Mirrors the Geometry contract in
/// `plans/091-cutout-card-shape-and-idle.md` and `src/styles.css`'s
/// `.card-assembly`/`.card-assembly.idle`/`.card-assembly.expanded`
/// rules. Any change to a width formula there MUST change the constants
/// at the top of this file — see
/// `active_card_rect_geometry_constants_match_named_style_constants`.
///
/// plan 091 (Decision 6, "no mode branch" in the shape itself): the
/// WIDTH FORMULA no longer branches on `mode` at all — idle/showing/
/// expanded use the exact same three formulas in both notch and HUD
/// mode now, mirroring `src/styles.css`'s own `.card-assembly` rules
/// (which read `var(--notchtap-cutout-width)` unconditionally, gated by
/// state classes, never by `[data-notchtap-mode]`). `mode` is used for
/// exactly one thing: resolving the cutout-width TERM those formulas
/// take as input — the measured hardware value in notch mode, or the
/// same `HUD_CUTOUT_W` synthetic constant App.tsx now sets the CSS var
/// to in HUD mode (previously HUD's `cutout_width` argument was simply
/// unused; leaving it unresolved here would under-measure the idle rect
/// in HUD mode, violating the CONSERVATIVE philosophy — see the 090
/// doc-comment paragraph below for why the term itself still can't be
/// scaled). This also fixes a pre-existing limitation the old
/// `Mode::Notch` arm had: today's rect ignored `visible`/`expanded`
/// entirely in notch mode (always the idle-clamped width even while a
/// card was showing) — Decision 6 removes that special case along with
/// the mode branch itself.
///
/// The vertical span is deliberately the FULL window height (no partial
/// `top`/`height` narrowing): unlike width, `styles.css` has no per-state
/// height breakpoints to mirror (091 exposes a notch HEIGHT var, but it
/// only sizes the flank ROW inside a card whose OUTER rect this function
/// still treats as the whole window), and the CONSERVATIVE philosophy
/// (never narrower than truth) permits skipping a height guess entirely
/// by covering the whole fixed 300px window. `css_top_down_to_appkit_y`
/// is still used for that span (not hardcoded `0.0..WINDOW_HEIGHT`
/// inline) so the one coordinate-flip seam stays in exactly one place.
///
/// `scale` is `Config.appearance.card_scale` (user-configurable via the
/// Settings Appearance section, default `1.0`) — a COSMETIC preference.
///
/// Plan 090 (Q1a), extended by 091: `scale` must NOT multiply the cutout
/// term in ANY mode now. In notch mode that term mirrors
/// `--notchtap-cutout-width`, itself a hardware measurement — the
/// physical `NSScreen` safe-area inset, read via the `notchtap-detect`
/// subprocess — not a design width; in HUD mode it's `HUD_CUTOUT_W`,
/// equally not a user-scalable design value (a notchless mac doesn't
/// grow a bigger synthetic notch because the user picked a bigger
/// card). Every OTHER width in these formulas (the flank px figures) IS
/// a cosmetic design width and IS multiplied by `scale`, matching
/// `styles.css`'s `var(--card-scale)` exactly. Do not "fix" either
/// cutout term back to multiplying by `scale` — that reverts a
/// deliberate, decided exemption, not an oversight
/// (`plans/090-card-scale-vs-hardware-geometry.md` has the original
/// rationale and the operator's Decision).
///
/// `_has_status_chips` is intentionally unused: plan 034's idle/idle-
/// status width split collapsed in 091 (the status dots replace the chip
/// rail entirely, so there is no wider idle variant to pick anymore) —
/// the parameter stays, prefixed, purely so the sole call site
/// (`lib.rs`'s `hover_point_is_over_card`) needs no edit; Rust's
/// positional call convention means the caller's variable name never
/// has to match this signature's.
pub fn active_card_rect(
    mode: Mode,
    cutout_width: f64,
    scale: f64,
    visible: bool,
    expanded: bool,
    _has_status_chips: bool,
) -> Rect {
    let effective_cutout_width = match mode {
        Mode::Notch => cutout_width,
        Mode::Hud => HUD_CUTOUT_W,
    };

    let raw_width = if visible && expanded {
        (BASE_EXPANDED * scale).max(effective_cutout_width + 2.0 * MIN_FLANK_SHOWING * scale)
    } else if visible {
        (BASE_SHOWING * scale).max(effective_cutout_width + 2.0 * MIN_FLANK_SHOWING * scale)
    } else {
        effective_cutout_width + 2.0 * FLANK_IDLE * scale
    };
    // the `min(..., 100%)` cap from the Geometry contract — `WINDOW_WIDTH`
    // is that "100%" in this window's own coordinate space.
    let width = raw_width.min(WINDOW_WIDTH);

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

    // --- active_card_rect: plan 091's three state formulas, HUD mode
    // (effective cutout = HUD_CUTOUT_W, always — the `cutout_width`
    // argument is irrelevant in this mode, pinned below), at scale 1.0 ---

    #[test]
    fn hud_idle_is_cutout_plus_two_flanks_at_scale_1() {
        let r = active_card_rect(Mode::Hud, 0.0, 1.0, false, false, false);
        assert_eq!(r.x_max - r.x_min, HUD_CUTOUT_W + 2.0 * FLANK_IDLE);
    }

    #[test]
    fn hud_showing_not_expanded_is_the_400_floor_at_scale_1() {
        // cutout(200) + 2*60 = 320, well under the 400 design floor, so
        // the floor wins — this is the common case (a real cutout is
        // never anywhere near 280px wide).
        let r = active_card_rect(Mode::Hud, 0.0, 1.0, true, false, false);
        assert_eq!(r.x_max - r.x_min, BASE_SHOWING);
    }

    #[test]
    fn hud_expanded_is_the_500_floor_at_scale_1() {
        let r = active_card_rect(Mode::Hud, 0.0, 1.0, true, true, false);
        assert_eq!(r.x_max - r.x_min, BASE_EXPANDED);
    }

    // plan 091: HUD mode always resolves the cutout term to
    // `HUD_CUTOUT_W` — the `cutout_width` argument passed in is simply
    // never consulted in this mode (lib.rs's caller happens to send
    // 0.0 for hud today; this test proves the result doesn't depend on
    // whatever it sends).
    #[test]
    fn hud_mode_ignores_the_passed_cutout_width_argument() {
        let with_zero = active_card_rect(Mode::Hud, 0.0, 1.0, false, false, false);
        let with_something_else = active_card_rect(Mode::Hud, 999.0, 1.0, false, false, false);
        assert_eq!(with_zero, with_something_else);
        assert_eq!(
            with_zero.x_max - with_zero.x_min,
            HUD_CUTOUT_W + 2.0 * FLANK_IDLE
        );
    }

    // --- the same three states at scale 0.8 and 1.25 — the flank/design
    // terms scale, the cutout term (HUD_CUTOUT_W here) never does. ---

    #[test]
    fn hud_idle_scales_the_flank_term_only_at_0_8() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.8, false, false, false);
        assert_eq!(r.x_max - r.x_min, HUD_CUTOUT_W + 2.0 * FLANK_IDLE * 0.8);
    }

    #[test]
    fn hud_idle_scales_the_flank_term_only_at_1_25() {
        let r = active_card_rect(Mode::Hud, 0.0, 1.25, false, false, false);
        assert_eq!(r.x_max - r.x_min, HUD_CUTOUT_W + 2.0 * FLANK_IDLE * 1.25);
    }

    #[test]
    fn hud_showing_scales_at_0_8() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.8, true, false, false);
        assert_eq!(
            r.x_max - r.x_min,
            (BASE_SHOWING * 0.8_f64).max(HUD_CUTOUT_W + 2.0 * MIN_FLANK_SHOWING * 0.8)
        );
    }

    #[test]
    fn hud_expanded_scales_at_0_8() {
        // 0.8, not 1.25: BASE_EXPANDED (500) already equals WINDOW_WIDTH
        // at scale 1.0, so any scale ABOVE 1.0 hits the window cap
        // immediately — that specific interaction has its own dedicated
        // test right below, `expanded_at_scale_above_1_hits_the_window_cap`.
        // This one isolates the scaling math itself, unaffected by the cap.
        let r = active_card_rect(Mode::Hud, 0.0, 0.8, true, true, false);
        assert_eq!(
            r.x_max - r.x_min,
            (BASE_EXPANDED * 0.8_f64).max(HUD_CUTOUT_W + 2.0 * MIN_FLANK_SHOWING * 0.8)
        );
    }

    // plan 091: a real, useful invariant this exposes — BASE_EXPANDED
    // (500) equals WINDOW_WIDTH (500) exactly, so the expanded state
    // hits its window cap at any scale above 1.0, in EITHER mode (a
    // user with `card_scale` > 1.0 always gets a full-window expanded
    // card, never wider).
    #[test]
    fn expanded_at_scale_above_1_hits_the_window_cap() {
        let hud = active_card_rect(Mode::Hud, 0.0, 1.25, true, true, false);
        assert_eq!(hud.x_max - hud.x_min, WINDOW_WIDTH);
        let notch = active_card_rect(Mode::Notch, 200.0, 1.25, true, true, false);
        assert_eq!(notch.x_max - notch.x_min, WINDOW_WIDTH);
    }

    // --- notch mode: the SAME three formulas, fed the measured cutout
    // (Decision 6 — "no mode branch" in the shape itself, so this is
    // deliberately not a separate code path, just a different input). ---

    #[test]
    fn notch_idle_is_measured_cutout_plus_two_flanks_at_scale_1() {
        // plan 063's own fixture (`src-tauri/src/lib.rs`'s
        // cutout_width_js_value test) — a realistic measured width.
        let r = active_card_rect(Mode::Notch, 319.0, 1.0, false, false, false);
        assert_eq!(r.x_max - r.x_min, 319.0 + 2.0 * FLANK_IDLE);
    }

    #[test]
    fn notch_showing_uses_the_measured_cutout_when_it_beats_the_floor() {
        // 319 + 2*60 = 439, which beats the 400 design floor — the
        // cutout-driven term wins here, unlike HUD's default 200px.
        let r = active_card_rect(Mode::Notch, 319.0, 1.0, true, false, false);
        assert_eq!(r.x_max - r.x_min, 319.0 + 2.0 * MIN_FLANK_SHOWING);
    }

    #[test]
    fn notch_expanded_falls_back_to_the_500_floor_when_the_cutout_is_narrower() {
        // 319 + 2*60 = 439, under the 500 expanded floor — the floor
        // wins here even though the same cutout beat the showing floor
        // above (400).
        let r = active_card_rect(Mode::Notch, 319.0, 1.0, true, true, false);
        assert_eq!(r.x_max - r.x_min, BASE_EXPANDED);
    }

    // plan 091: the cutout term stays unscaled in notch mode too — but
    // unlike the old design (where the whole notch-mode rect was
    // scale-invariant, since flanks never scaled there at all), the
    // FLANK term now scales in every mode (Decision 6). This isolates
    // just the cutout term's exemption: the scale-1.0-to-1.25 delta must
    // equal exactly the flank term's own delta, with nothing attributed
    // to the 200px cutout figure. (200, not 319, so the result stays
    // under the WINDOW_WIDTH cap at both scales — see the cap tests
    // below for what happens when it doesn't.
    #[test]
    fn notch_mode_cutout_term_stays_unscaled_only_the_flank_term_scales() {
        let at_scale_1 = active_card_rect(Mode::Notch, 200.0, 1.0, false, false, false);
        let at_scale_1_25 = active_card_rect(Mode::Notch, 200.0, 1.25, false, false, false);
        let width_1 = at_scale_1.x_max - at_scale_1.x_min;
        let width_1_25 = at_scale_1_25.x_max - at_scale_1_25.x_min;
        let expected_flank_delta = 2.0 * FLANK_IDLE * (1.25 - 1.0);
        assert!(
            (width_1_25 - width_1 - expected_flank_delta).abs() < 1e-9,
            "width_1={width_1}, width_1_25={width_1_25}, expected_flank_delta={expected_flank_delta}"
        );
    }

    // --- the `min(..., 100%)` cap (Geometry contract) — `WINDOW_WIDTH`
    // in this window's own coordinate space. A wide-enough measured
    // cutout can otherwise exceed the window, which must never happen. ---

    #[test]
    fn notch_idle_caps_at_the_window_width_for_a_very_wide_cutout() {
        let r = active_card_rect(Mode::Notch, 600.0, 1.0, false, false, false);
        assert_eq!(r.x_max - r.x_min, WINDOW_WIDTH);
    }

    #[test]
    fn notch_showing_caps_at_the_window_width_for_a_very_wide_cutout() {
        let r = active_card_rect(Mode::Notch, 600.0, 1.0, true, false, false);
        assert_eq!(r.x_max - r.x_min, WINDOW_WIDTH);
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

    // Mirrors src/styles.css's `.card-assembly` (idle: FLANK_IDLE),
    // `.card-assembly`/`.card-assembly.expanded` (showing/expanded:
    // MIN_FLANK_SHOWING + BASE_SHOWING/BASE_EXPANDED), and App.tsx's HUD
    // synthetic constants (HUD_CUTOUT_W/HUD_CUTOUT_H) — a NAMED-constant
    // assertion, not a live CSS parse (spike §6's explicit
    // simplification, carried forward by plan 091). If a future edit
    // changes one of these numbers in styles.css or App.tsx without
    // updating the constants at the top of this file, this test does NOT
    // catch it by itself (it only asserts internal self-consistency) —
    // it exists so a reviewer diffing this file sees the citations and
    // checks both sides. plan 091: replaces the old BASE_WIDTH/
    // EXPANDED_WIDTH/IDLE_WIDTH/IDLE_STATUS_WIDTH/NOTCH_CLAMP_MIN/
    // NOTCH_CLAMP_MAX set (see the constants' own doc comments for why
    // each was removed).
    #[test]
    fn active_card_rect_geometry_constants_match_named_style_constants() {
        assert_eq!(FLANK_IDLE, 85.0);
        assert_eq!(MIN_FLANK_SHOWING, 60.0);
        assert_eq!(BASE_SHOWING, 400.0);
        assert_eq!(BASE_EXPANDED, 500.0);
        assert_eq!(HUD_CUTOUT_W, 200.0);
        assert_eq!(HUD_CUTOUT_H, 32.0);
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
