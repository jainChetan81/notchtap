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
const HUD_CUTOUT_H: f64 = 32.0; // App.tsx's HUD synthetic cutout height

// plan 093: the y-span constants. `IDLE_PEEK_BELOW_BLOCK_H` is a REAL
// duplicated-constant (styles.css's `.idle-peek` fixed target height —
// unlike the showing/expanded below-block, which is CSS `auto`-sized off
// real content, the peek is a deliberately FIXED-height block, same
// technique as the locked reference's own `.wx-peek { height: 78px }`,
// `prototype/notch-states.html:119`) — any change to one MUST change the
// other in the same commit, same discipline as every other constant
// above.
//
// `BELOW_BLOCK_SHOWING_H`/`BELOW_BLOCK_EXPANDED_H` are NOT duplicated
// constants in that sense — the real showing/expanded below-block is CSS
// `auto`-height, sized by whatever content the card carries (a short
// compact body vs. a long news article vs. a full manifest), so there is
// no single true number in styles.css to mirror. These are deliberately
// CONSERVATIVE ESTIMATES (never intended to be pixel-exact) chosen to
// close most of the ~240px dead-zone gap 079 item 17 flagged while
// staying safely UNDER what a real card of that kind renders — the
// hover rect must never claim MORE height than the card actually
// occupies, per this file's standing CONSERVATIVE philosophy (err
// small, not generous, when in doubt). If a future redesign changes the
// compact/manifest content shape enough to make these feel wrong,
// adjust them directly; there is no styles.css number to keep them "in
// sync" with.
const IDLE_PEEK_BELOW_BLOCK_H: f64 = 100.0; // IdleHoverPeek.tsx motion.div `animate={{ height: 100 }}` — lockstep pair (was styles.css pre-motion-migration)
const BELOW_BLOCK_SHOWING_H: f64 = 160.0; // conservative estimate, compact (non-expanded) content
const BELOW_BLOCK_EXPANDED_H: f64 = 240.0; // conservative estimate, expanded (manifest) content

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
/// The vertical span (plan 093 — replaces the pre-093 "always the full
/// window" behavior 091 shipped, flagged in its own now-deleted comment
/// here as a carried-forward limitation, and in 079 item 17's bracketed
/// note as "~240px of empty space below the card currently registers as
/// hovered"): `top` is always `0.0` (`.card-assembly` has no vertical
/// margin — `src/styles.css`'s `html,body,#root { margin:0 }` plus
/// `.card-assembly`'s own `margin: 0 auto` centers horizontally only),
/// and `height` is derived from the actual assembly state rather than
/// hardcoded to `WINDOW_HEIGHT`:
/// - idle, peek/reveal closed: `effective_cutout_height` alone — during
///   idle, `.card-assembly`'s grid row 2 (`auto`) has no content in it at
///   all (no below-block mounted), so its real rendered height IS
///   exactly the cutout row's height, not an estimate.
/// - idle, peek/reveal open (`idle_peek_open`): cutout height +
///   `IDLE_PEEK_BELOW_BLOCK_H` — the hover-expanded idle state (plan 093:
///   the weather peek / scorecard reveal / day-progress timeline), whose
///   below-block is a real, FIXED-height CSS block (`styles.css`'s
///   `.idle-peek`), mirrored here exactly like every other duplicated
///   width constant above.
/// - showing (not expanded): cutout height + `BELOW_BLOCK_SHOWING_H`.
/// - expanded: cutout height + `BELOW_BLOCK_EXPANDED_H`.
///
/// Both non-idle estimates are deliberately CONSERVATIVE (see those
/// constants' own doc comments) — this function still never claims to be
/// pixel-perfect, only close enough to kill the ~240px dead zone. The
/// total is capped at `WINDOW_HEIGHT`, the same `min(..., 100%)`
/// discipline the width formula already uses, via the same
/// `css_top_down_to_appkit_y` flip (never hardcoded inline) so the one
/// coordinate-flip seam stays in exactly one place.
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
/// rationale and the operator's Decision). `cutout_height` (plan 093,
/// notch mode only — `HUD_CUTOUT_H` is a synthetic constant in HUD mode,
/// never a measured term) follows the exact same exemption for the exact
/// same reason: a hardware/synthetic measurement, never a cosmetic
/// design height, so it is likewise never multiplied by `scale`.
///
/// `idle_peek_open` (plan 093: replaces the formerly-unused
/// `_has_status_chips` slot — plan 034's idle/idle-status WIDTH split it
/// was named for collapsed in 091, and this plan repurposes the spare
/// boolean rather than adding an 8th positional parameter) is
/// deliberately NOT "is there weather/live-match data available" — it is
/// hover HYSTERESIS: "as of the last computed frame, was the cursor
/// already registered as hovering." `lib.rs`'s `hover_point_is_over_card`
/// passes in `was_hovered`'s CURRENT value (read before this event can
/// overwrite it). This is what lets the rect GROW to cover the peek's
/// newly-opened area once hover starts (so moving the cursor further
/// down, into the area that only just became visible, doesn't
/// immediately fall outside the rect and snap the peek shut), while
/// staying at the tight cutout-only height the rest of the time (idle,
/// not hovered — the overwhelming majority of an idle card's lifetime).
/// The idle-hover-expanded state itself is unconditional on ambient data
/// (item 18's decision: the day-progress timeline lives here regardless
/// of whether weather/football happen to be configured) — see
/// `src/components/IdleHoverPeek.tsx` for what actually renders inside
/// it. Only relevant while `!visible`; ignored (never read) whenever
/// `visible` is `true`.
pub fn active_card_rect(
    mode: Mode,
    cutout_width: f64,
    cutout_height: f64,
    scale: f64,
    visible: bool,
    expanded: bool,
    idle_peek_open: bool,
) -> Rect {
    let effective_cutout_width = match mode {
        Mode::Notch => cutout_width,
        Mode::Hud => HUD_CUTOUT_W,
    };
    let effective_cutout_height = match mode {
        Mode::Notch => cutout_height,
        Mode::Hud => HUD_CUTOUT_H,
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

    let below_block_h = if !visible {
        if idle_peek_open {
            IDLE_PEEK_BELOW_BLOCK_H
        } else {
            0.0
        }
    } else if expanded {
        BELOW_BLOCK_EXPANDED_H
    } else {
        BELOW_BLOCK_SHOWING_H
    };
    let raw_height = effective_cutout_height + below_block_h;
    let height = raw_height.min(WINDOW_HEIGHT);

    let x_min = (WINDOW_WIDTH - width) / 2.0;
    let (y_min, y_max) = css_top_down_to_appkit_y(WINDOW_HEIGHT, 0.0, height);

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

    // --- active_card_rect: plan 091's three state formulas, HUD mode
    // (effective cutout = HUD_CUTOUT_W, always — the `cutout_width`
    // argument is irrelevant in this mode, pinned below), at scale 1.0 ---

    #[test]
    fn hud_idle_is_cutout_plus_two_flanks_at_scale_1() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, false);
        assert_eq!(r.x_max - r.x_min, HUD_CUTOUT_W + 2.0 * FLANK_IDLE);
    }

    #[test]
    fn hud_showing_not_expanded_is_the_400_floor_at_scale_1() {
        // cutout(200) + 2*60 = 320, well under the 400 design floor, so
        // the floor wins — this is the common case (a real cutout is
        // never anywhere near 280px wide).
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false);
        assert_eq!(r.x_max - r.x_min, BASE_SHOWING);
    }

    #[test]
    fn hud_expanded_is_the_500_floor_at_scale_1() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, false);
        assert_eq!(r.x_max - r.x_min, BASE_EXPANDED);
    }

    // plan 091: HUD mode always resolves the cutout term to
    // `HUD_CUTOUT_W` — the `cutout_width` argument passed in is simply
    // never consulted in this mode (lib.rs's caller happens to send
    // 0.0 for hud today; this test proves the result doesn't depend on
    // whatever it sends).
    #[test]
    fn hud_mode_ignores_the_passed_cutout_width_argument() {
        let with_zero = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, false);
        let with_something_else = active_card_rect(Mode::Hud, 999.0, 0.0, 1.0, false, false, false);
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
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 0.8, false, false, false);
        assert_eq!(r.x_max - r.x_min, HUD_CUTOUT_W + 2.0 * FLANK_IDLE * 0.8);
    }

    #[test]
    fn hud_idle_scales_the_flank_term_only_at_1_25() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.25, false, false, false);
        assert_eq!(r.x_max - r.x_min, HUD_CUTOUT_W + 2.0 * FLANK_IDLE * 1.25);
    }

    #[test]
    fn hud_showing_scales_at_0_8() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 0.8, true, false, false);
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
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 0.8, true, true, false);
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
        let hud = active_card_rect(Mode::Hud, 0.0, 0.0, 1.25, true, true, false);
        assert_eq!(hud.x_max - hud.x_min, WINDOW_WIDTH);
        let notch = active_card_rect(Mode::Notch, 200.0, 32.0, 1.25, true, true, false);
        assert_eq!(notch.x_max - notch.x_min, WINDOW_WIDTH);
    }

    // --- notch mode: the SAME three formulas, fed the measured cutout
    // (Decision 6 — "no mode branch" in the shape itself, so this is
    // deliberately not a separate code path, just a different input). ---

    #[test]
    fn notch_idle_is_measured_cutout_plus_two_flanks_at_scale_1() {
        // plan 063's own fixture (`src-tauri/src/lib.rs`'s
        // cutout_width_js_value test) — a realistic measured width.
        let r = active_card_rect(Mode::Notch, 319.0, 32.0, 1.0, false, false, false);
        assert_eq!(r.x_max - r.x_min, 319.0 + 2.0 * FLANK_IDLE);
    }

    #[test]
    fn notch_showing_uses_the_measured_cutout_when_it_beats_the_floor() {
        // 319 + 2*60 = 439, which beats the 400 design floor — the
        // cutout-driven term wins here, unlike HUD's default 200px.
        let r = active_card_rect(Mode::Notch, 319.0, 32.0, 1.0, true, false, false);
        assert_eq!(r.x_max - r.x_min, 319.0 + 2.0 * MIN_FLANK_SHOWING);
    }

    #[test]
    fn notch_expanded_falls_back_to_the_500_floor_when_the_cutout_is_narrower() {
        // 319 + 2*60 = 439, under the 500 expanded floor — the floor
        // wins here even though the same cutout beat the showing floor
        // above (400).
        let r = active_card_rect(Mode::Notch, 319.0, 32.0, 1.0, true, true, false);
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
        let at_scale_1 = active_card_rect(Mode::Notch, 200.0, 32.0, 1.0, false, false, false);
        let at_scale_1_25 = active_card_rect(Mode::Notch, 200.0, 32.0, 1.25, false, false, false);
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
        let r = active_card_rect(Mode::Notch, 600.0, 32.0, 1.0, false, false, false);
        assert_eq!(r.x_max - r.x_min, WINDOW_WIDTH);
    }

    #[test]
    fn notch_showing_caps_at_the_window_width_for_a_very_wide_cutout() {
        let r = active_card_rect(Mode::Notch, 600.0, 32.0, 1.0, true, false, false);
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
        // plan 093: IDLE_PEEK_BELOW_BLOCK_H is a real duplicated-constant
        // (styles.css's `.idle-peek` fixed height) — see its own doc
        // comment for why BELOW_BLOCK_SHOWING_H/BELOW_BLOCK_EXPANDED_H
        // are deliberately NOT asserted here (they're estimates, not a
        // styles.css mirror).
        assert_eq!(IDLE_PEEK_BELOW_BLOCK_H, 100.0);
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

    // plan 093: was `active_card_rect_y_span_is_the_full_window_height`,
    // pinning the pre-093 "always the whole window" behavior — UPDATED,
    // not deleted, per the plan's explicit instruction. Idle, peek
    // closed: the y-span is now the cutout row's height alone, nowhere
    // near the full 300px window — the headline fix this plan exists for.
    #[test]
    fn idle_peek_closed_y_span_is_the_cutout_height_alone() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, false);
        let height = r.y_max - r.y_min;
        assert_eq!(height, HUD_CUTOUT_H);
        assert!(
            height < WINDOW_HEIGHT,
            "the whole point of plan 093's y-span fix: idle must not span the full window"
        );
    }

    // --- plan 093: the y-span's height term, one case per assembly state ---

    #[test]
    fn idle_peek_open_y_span_adds_the_peek_below_block_height() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, true);
        assert_eq!(r.y_max - r.y_min, HUD_CUTOUT_H + IDLE_PEEK_BELOW_BLOCK_H);
    }

    #[test]
    fn notch_idle_peek_closed_y_span_uses_the_measured_cutout_height() {
        let r = active_card_rect(Mode::Notch, 319.0, 40.0, 1.0, false, false, false);
        assert_eq!(r.y_max - r.y_min, 40.0);
    }

    #[test]
    fn notch_idle_peek_open_y_span_uses_the_measured_cutout_height_plus_peek() {
        let r = active_card_rect(Mode::Notch, 319.0, 40.0, 1.0, false, false, true);
        assert_eq!(r.y_max - r.y_min, 40.0 + IDLE_PEEK_BELOW_BLOCK_H);
    }

    #[test]
    fn showing_not_expanded_y_span_adds_the_showing_below_block_estimate() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false);
        assert_eq!(r.y_max - r.y_min, HUD_CUTOUT_H + BELOW_BLOCK_SHOWING_H);
    }

    #[test]
    fn expanded_y_span_adds_the_expanded_below_block_estimate() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, false);
        assert_eq!(r.y_max - r.y_min, HUD_CUTOUT_H + BELOW_BLOCK_EXPANDED_H);
    }

    // idle_peek_open is documented as irrelevant once `visible` is true —
    // prove it the same way `hud_mode_ignores_the_passed_cutout_width_
    // argument` proves the analogous width-side claim.
    #[test]
    fn idle_peek_open_is_ignored_while_visible() {
        let showing_false = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false);
        let showing_true = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, true);
        assert_eq!(showing_false, showing_true);
        let expanded_false = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, false);
        let expanded_true = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, true);
        assert_eq!(expanded_false, expanded_true);
    }

    // the height side of the `min(..., 100%)` cap — mirrors the width
    // cap tests above. A tall enough measured cutout (or, in principle, a
    // tall enough below-block estimate) must never push the rect past
    // the fixed window.
    #[test]
    fn height_caps_at_the_window_height_for_a_tall_measured_cutout() {
        let r = active_card_rect(Mode::Notch, 200.0, 280.0, 1.0, true, true, false);
        assert_eq!(r.y_max - r.y_min, WINDOW_HEIGHT);
    }

    // --- the actual behavioral fix: a point in the old dead zone below
    // the idle card no longer registers as hovered. ---

    #[test]
    fn idle_peek_closed_point_below_the_cutout_row_is_not_in_the_rect() {
        // HUD idle: cutout height alone is 32.0. A point comfortably
        // inside the OLD full-300px span but below the real 32px-tall
        // card (in CSS top-down terms, y=100 — i.e. AppKit y = 300-100 =
        // 200) must no longer count as hovered.
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, false);
        let (appkit_y_min, appkit_y_max) = css_top_down_to_appkit_y(WINDOW_HEIGHT, 0.0, 32.0);
        assert_eq!((appkit_y_min, appkit_y_max), (268.0, 300.0));
        // AppKit y=200 is well below the idle rect's low edge (268) — the
        // dead zone the pre-093 full-window rect used to wrongly cover.
        assert!(!point_in_rect(&r, WINDOW_WIDTH / 2.0, 200.0));
        // sanity: a point actually inside the real idle rect still hovers.
        assert!(point_in_rect(&r, WINDOW_WIDTH / 2.0, 280.0));
    }
}
