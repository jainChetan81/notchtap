//! The hover primitive's pure geometry/state logic — no AppKit types anywhere in this module.

use crate::presentation::Mode;

/// The fixed overlay window's size — `src-tauri/tauri.conf.json`'s `"width": 500, "height": 300`,
/// `"resizable": false`.
pub(crate) const WINDOW_WIDTH: f64 = 500.0;
pub(crate) const WINDOW_HEIGHT: f64 = 300.0;

const FLANK_IDLE: f64 = 85.0; // idle flank width, styles.css .card-assembly.idle
const MIN_FLANK_SHOWING: f64 = 60.0; // showing/expanded minimum flank width
const BASE_SHOWING: f64 = 400.0; // .card-assembly (showing) design-width floor
const BASE_EXPANDED: f64 = 500.0; // .card-assembly.expanded design-width floor
const HUD_CUTOUT_W: f64 = 200.0; // App.tsx's HUD synthetic cutout width
const HUD_CUTOUT_H: f64 = 32.0; // App.tsx's HUD synthetic cutout height

// `IDLE_PEEK_BELOW_BLOCK_H` is a REAL duplicated constant (the peek is a deliberately FIXED-height
// block) — any change to one copy MUST change the other in the same commit.
const IDLE_PEEK_BELOW_BLOCK_H: f64 = 100.0; // IdleHoverPeek.tsx motion.div `animate={{ height: 100 }}` — lockstep pair
const BELOW_BLOCK_SHOWING_H: f64 = 160.0; // conservative estimate, compact (non-expanded) content
const BELOW_BLOCK_EXPANDED_H: f64 = 240.0; // conservative estimate, expanded (manifest) content

// Icon-strip geometry — LOCKSTEP PAIRS with real CSS.
const ICON_BOX: f64 = 18.0;
const ICON_GAP: f64 = 8.0;
const FLANK_INSET: f64 = 16.0;

fn hovered_right_flank_width(present_count: usize, scale: f64) -> f64 {
    let strip_w = (ICON_BOX + ICON_GAP) * present_count as f64 + FLANK_INSET;
    (FLANK_IDLE * scale).max(strip_w)
}

pub fn icon_strip_rects(
    mode: Mode,
    cutout_width: f64,
    cutout_height: f64,
    scale: f64,
    present_count: usize,
    window_height: f64,
) -> Vec<Rect> {
    if present_count == 0 {
        return Vec::new();
    }
    let effective_cutout_width = match mode {
        Mode::Notch => cutout_width,
        Mode::Hud => HUD_CUTOUT_W,
    };
    let effective_cutout_height = match mode {
        Mode::Notch => cutout_height,
        Mode::Hud => HUD_CUTOUT_H,
    };
    let flank_w = hovered_right_flank_width(present_count, scale);
    let total_width = (effective_cutout_width + 2.0 * flank_w).min(WINDOW_WIDTH);
    let card_x_min = (WINDOW_WIDTH - total_width) / 2.0;
    let card_x_max = card_x_min + total_width;

    let (y_min, y_max) = css_top_down_to_appkit_y(window_height, 0.0, effective_cutout_height);

    (0..present_count)
        .map(|from_right| {
            let right_edge = card_x_max - FLANK_INSET - from_right as f64 * (ICON_BOX + ICON_GAP);
            let left_edge = right_edge - ICON_BOX;
            Rect {
                x_min: left_edge,
                x_max: right_edge,
                y_min,
                y_max,
            }
        })
        .rev()
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

/// Inclusive-bounds point-in-rect test — a point exactly on an edge counts as inside (matches the
/// CONSERVATIVE philosophy: never narrower than the true rendered edge).
pub fn point_in_rect(rect: &Rect, x: f64, y: f64) -> bool {
    x >= rect.x_min && x <= rect.x_max && y >= rect.y_min && y <= rect.y_max
}

/// The coordinate-space flip. Getting this backwards is silent — a y-comparison never panics, it
/// just stops matching the rendered card.
pub fn css_top_down_to_appkit_y(window_height: f64, top: f64, height: f64) -> (f64, f64) {
    let high = window_height - top;
    let low = high - height;
    (low, high)
}

/// Deliberately CONSERVATIVE: it may be slightly wider than the true rendered edge, never narrower.
// The 8th parameter crosses clippy's default 7-arg threshold. Same call
// as `lib.rs`'s `hover_point_is_over_card`/
// `emit_hover_changed_if_transitioned` (which carry the same `#[allow]`
// for the same reason): a named-field params struct is a bigger surface
// change than this fix's scope for a pure geometry function whose whole
// non-test call graph is one expression in `lib.rs`.
#[allow(clippy::too_many_arguments)]
pub fn active_card_rect(
    mode: Mode,
    cutout_width: f64,
    cutout_height: f64,
    scale: f64,
    visible: bool,
    expanded: bool,
    idle_peek_open: bool,
    hover_expand_open: bool,
) -> Rect {
    let effective_cutout_width = match mode {
        Mode::Notch => cutout_width,
        Mode::Hud => HUD_CUTOUT_W,
    };
    let effective_cutout_height = match mode {
        Mode::Notch => cutout_height,
        Mode::Hud => HUD_CUTOUT_H,
    };

    let expanded_geometry = expanded || hover_expand_open;

    let raw_width = if visible && expanded_geometry {
        (BASE_EXPANDED * scale).max(effective_cutout_width + 2.0 * MIN_FLANK_SHOWING * scale)
    } else if visible {
        (BASE_SHOWING * scale).max(effective_cutout_width + 2.0 * MIN_FLANK_SHOWING * scale)
    } else {
        effective_cutout_width + 2.0 * FLANK_IDLE * scale
    };
    let width = raw_width.min(WINDOW_WIDTH);

    let below_block_h = if !visible {
        if idle_peek_open {
            IDLE_PEEK_BELOW_BLOCK_H
        } else {
            0.0
        }
    } else if expanded_geometry {
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

const BOARD_PRIMARY_H: f64 = 150.0; // conservative estimate, agent-board.css's `.agent-board-primary` block
const BOARD_ROW_H: f64 = 18.0; // conservative estimate, agent-board.css's `.agent-row`

/// `window_height` must be the currently applied native height, not the resting constant.
pub fn board_rect(
    mode: Mode,
    cutout_width: f64,
    cutout_height: f64,
    scale: f64,
    session_count: usize,
    window_height: f64,
) -> Rect {
    let effective_cutout_width = match mode {
        Mode::Notch => cutout_width,
        Mode::Hud => HUD_CUTOUT_W,
    };
    let effective_cutout_height = match mode {
        Mode::Notch => cutout_height,
        Mode::Hud => HUD_CUTOUT_H,
    };

    let raw_width =
        (BASE_EXPANDED * scale).max(effective_cutout_width + 2.0 * MIN_FLANK_SHOWING * scale);
    let width = raw_width.min(WINDOW_WIDTH);

    let extra_rows = session_count.saturating_sub(1);
    let below_block_h = BOARD_PRIMARY_H + BOARD_ROW_H * extra_rows as f64;
    let raw_height = effective_cutout_height + below_block_h;
    let height = raw_height.min(window_height);

    let x_min = (WINDOW_WIDTH - width) / 2.0;
    let (y_min, y_max) = css_top_down_to_appkit_y(window_height, 0.0, height);

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

    #[test]
    fn hud_idle_is_cutout_plus_two_flanks_at_scale_1() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, false, false);
        assert_eq!(r.x_max - r.x_min, HUD_CUTOUT_W + 2.0 * FLANK_IDLE);
    }

    #[test]
    fn hud_showing_not_expanded_is_the_400_floor_at_scale_1() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false, false);
        assert_eq!(r.x_max - r.x_min, BASE_SHOWING);
    }

    #[test]
    fn hud_expanded_is_the_500_floor_at_scale_1() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, false, false);
        assert_eq!(r.x_max - r.x_min, BASE_EXPANDED);
    }

    #[test]
    fn hud_mode_ignores_the_passed_cutout_width_argument() {
        let with_zero = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, false, false);
        let with_something_else =
            active_card_rect(Mode::Hud, 999.0, 0.0, 1.0, false, false, false, false);
        assert_eq!(with_zero, with_something_else);
        assert_eq!(
            with_zero.x_max - with_zero.x_min,
            HUD_CUTOUT_W + 2.0 * FLANK_IDLE
        );
    }

    #[test]
    fn hud_idle_scales_the_flank_term_only_at_0_8() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 0.8, false, false, false, false);
        assert_eq!(r.x_max - r.x_min, HUD_CUTOUT_W + 2.0 * FLANK_IDLE * 0.8);
    }

    #[test]
    fn hud_idle_scales_the_flank_term_only_at_1_25() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.25, false, false, false, false);
        assert_eq!(r.x_max - r.x_min, HUD_CUTOUT_W + 2.0 * FLANK_IDLE * 1.25);
    }

    #[test]
    fn hud_showing_scales_at_0_8() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 0.8, true, false, false, false);
        assert_eq!(
            r.x_max - r.x_min,
            (BASE_SHOWING * 0.8_f64).max(HUD_CUTOUT_W + 2.0 * MIN_FLANK_SHOWING * 0.8)
        );
    }

    #[test]
    fn hud_expanded_scales_at_0_8() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 0.8, true, true, false, false);
        assert_eq!(
            r.x_max - r.x_min,
            (BASE_EXPANDED * 0.8_f64).max(HUD_CUTOUT_W + 2.0 * MIN_FLANK_SHOWING * 0.8)
        );
    }

    #[test]
    fn expanded_at_scale_above_1_hits_the_window_cap() {
        let hud = active_card_rect(Mode::Hud, 0.0, 0.0, 1.25, true, true, false, false);
        assert_eq!(hud.x_max - hud.x_min, WINDOW_WIDTH);
        let notch = active_card_rect(Mode::Notch, 200.0, 32.0, 1.25, true, true, false, false);
        assert_eq!(notch.x_max - notch.x_min, WINDOW_WIDTH);
    }

    #[test]
    fn notch_idle_is_measured_cutout_plus_two_flanks_at_scale_1() {
        let r = active_card_rect(Mode::Notch, 319.0, 32.0, 1.0, false, false, false, false);
        assert_eq!(r.x_max - r.x_min, 319.0 + 2.0 * FLANK_IDLE);
    }

    #[test]
    fn notch_showing_uses_the_measured_cutout_when_it_beats_the_floor() {
        let r = active_card_rect(Mode::Notch, 319.0, 32.0, 1.0, true, false, false, false);
        assert_eq!(r.x_max - r.x_min, 319.0 + 2.0 * MIN_FLANK_SHOWING);
    }

    #[test]
    fn notch_expanded_falls_back_to_the_500_floor_when_the_cutout_is_narrower() {
        let r = active_card_rect(Mode::Notch, 319.0, 32.0, 1.0, true, true, false, false);
        assert_eq!(r.x_max - r.x_min, BASE_EXPANDED);
    }

    #[test]
    fn notch_mode_cutout_term_stays_unscaled_only_the_flank_term_scales() {
        let at_scale_1 =
            active_card_rect(Mode::Notch, 200.0, 32.0, 1.0, false, false, false, false);
        let at_scale_1_25 =
            active_card_rect(Mode::Notch, 200.0, 32.0, 1.25, false, false, false, false);
        let width_1 = at_scale_1.x_max - at_scale_1.x_min;
        let width_1_25 = at_scale_1_25.x_max - at_scale_1_25.x_min;
        let expected_flank_delta = 2.0 * FLANK_IDLE * (1.25 - 1.0);
        assert!(
            (width_1_25 - width_1 - expected_flank_delta).abs() < 1e-9,
            "width_1={width_1}, width_1_25={width_1_25}, expected_flank_delta={expected_flank_delta}"
        );
    }

    #[test]
    fn notch_idle_caps_at_the_window_width_for_a_very_wide_cutout() {
        let r = active_card_rect(Mode::Notch, 600.0, 32.0, 1.0, false, false, false, false);
        assert_eq!(r.x_max - r.x_min, WINDOW_WIDTH);
    }

    #[test]
    fn notch_showing_caps_at_the_window_width_for_a_very_wide_cutout() {
        let r = active_card_rect(Mode::Notch, 600.0, 32.0, 1.0, true, false, false, false);
        assert_eq!(r.x_max - r.x_min, WINDOW_WIDTH);
    }

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

    #[test]
    fn active_card_rect_geometry_constants_match_named_style_constants() {
        assert_eq!(FLANK_IDLE, 85.0);
        assert_eq!(MIN_FLANK_SHOWING, 60.0);
        assert_eq!(BASE_SHOWING, 400.0);
        assert_eq!(BASE_EXPANDED, 500.0);
        assert_eq!(HUD_CUTOUT_W, 200.0);
        assert_eq!(HUD_CUTOUT_H, 32.0);
        assert_eq!(IDLE_PEEK_BELOW_BLOCK_H, 100.0);
    }

    #[test]
    fn top_of_window_maps_to_high_appkit_y_not_low() {
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
    fn idle_peek_closed_y_span_is_the_cutout_height_alone() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, false, false);
        let height = r.y_max - r.y_min;
        assert_eq!(height, HUD_CUTOUT_H);
        assert!(height < WINDOW_HEIGHT, "idle must not span the full window");
    }

    #[test]
    fn idle_peek_open_y_span_adds_the_peek_below_block_height() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, true, false);
        assert_eq!(r.y_max - r.y_min, HUD_CUTOUT_H + IDLE_PEEK_BELOW_BLOCK_H);
    }

    #[test]
    fn notch_idle_peek_closed_y_span_uses_the_measured_cutout_height() {
        let r = active_card_rect(Mode::Notch, 319.0, 40.0, 1.0, false, false, false, false);
        assert_eq!(r.y_max - r.y_min, 40.0);
    }

    #[test]
    fn notch_idle_peek_open_y_span_uses_the_measured_cutout_height_plus_peek() {
        let r = active_card_rect(Mode::Notch, 319.0, 40.0, 1.0, false, false, true, false);
        assert_eq!(r.y_max - r.y_min, 40.0 + IDLE_PEEK_BELOW_BLOCK_H);
    }

    #[test]
    fn showing_not_expanded_y_span_adds_the_showing_below_block_estimate() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false, false);
        assert_eq!(r.y_max - r.y_min, HUD_CUTOUT_H + BELOW_BLOCK_SHOWING_H);
    }

    #[test]
    fn expanded_y_span_adds_the_expanded_below_block_estimate() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, false, false);
        assert_eq!(r.y_max - r.y_min, HUD_CUTOUT_H + BELOW_BLOCK_EXPANDED_H);
    }

    #[test]
    fn idle_peek_open_is_ignored_while_visible() {
        let showing_false = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false, false);
        let showing_true = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, true, false);
        assert_eq!(showing_false, showing_true);
        let expanded_false = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, false, false);
        let expanded_true = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, true, false);
        assert_eq!(expanded_false, expanded_true);
    }

    #[test]
    fn height_caps_at_the_window_height_for_a_tall_measured_cutout() {
        let r = active_card_rect(Mode::Notch, 200.0, 280.0, 1.0, true, true, false, false);
        assert_eq!(r.y_max - r.y_min, WINDOW_HEIGHT);
    }

    #[test]
    fn idle_peek_closed_point_below_the_cutout_row_is_not_in_the_rect() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, false, false);
        let (appkit_y_min, appkit_y_max) = css_top_down_to_appkit_y(WINDOW_HEIGHT, 0.0, 32.0);
        assert_eq!((appkit_y_min, appkit_y_max), (268.0, 300.0));
        assert!(!point_in_rect(&r, WINDOW_WIDTH / 2.0, 200.0));
        assert!(point_in_rect(&r, WINDOW_WIDTH / 2.0, 280.0));
    }

    #[test]
    fn hover_expand_open_widens_a_showing_card_to_the_expanded_width() {
        let compact = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false, false);
        let hover_expanded = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false, true);
        assert_eq!(compact.x_max - compact.x_min, BASE_SHOWING);
        assert_eq!(
            hover_expanded.x_max - hover_expanded.x_min,
            BASE_EXPANDED,
            "a hovered showing card paints at the expanded width, so the rect must too"
        );
    }

    #[test]
    fn hover_expand_open_matches_the_manually_expanded_rect_exactly() {
        let manually = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, false, false);
        let by_hover = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false, true);
        assert_eq!(manually, by_hover);
        let manually_notch =
            active_card_rect(Mode::Notch, 319.0, 32.0, 1.0, true, true, false, false);
        let by_hover_notch =
            active_card_rect(Mode::Notch, 319.0, 32.0, 1.0, true, false, false, true);
        assert_eq!(manually_notch, by_hover_notch);
    }

    #[test]
    fn hover_expand_open_adds_the_expanded_below_block_height() {
        let r = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false, true);
        assert_eq!(r.y_max - r.y_min, HUD_CUTOUT_H + BELOW_BLOCK_EXPANDED_H);
    }

    #[test]
    fn hover_expand_open_is_ignored_while_not_visible() {
        let idle_false = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, false, false);
        let idle_true = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, false, true);
        assert_eq!(idle_false, idle_true);
        let peek_false = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, true, false);
        let peek_true = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, false, false, true, true);
        assert_eq!(peek_false, peek_true);
    }

    #[test]
    fn hover_expand_open_is_a_no_op_on_an_already_expanded_card() {
        let expanded_only = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, false, false);
        let expanded_and_hovered =
            active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, false, true);
        assert_eq!(expanded_only, expanded_and_hovered);
    }

    #[test]
    fn a_point_in_the_hover_revealed_manifest_stays_inside_the_grown_rect() {
        let compact = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false, false);
        let grown = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, false, false, true);
        let (_, appkit_y) = css_top_down_to_appkit_y(WINDOW_HEIGHT, 250.0, 0.0);
        assert!(
            !point_in_rect(&compact, WINDOW_WIDTH / 2.0, appkit_y),
            "the revealed manifest must be hoverable, not just painted"
        );
        assert!(
            point_in_rect(&grown, WINDOW_WIDTH / 2.0, appkit_y),
            "with the latch set the cursor stays inside, so the card cannot collapse under it"
        );
        let flank_x = compact.x_max + 25.0;
        assert!(!point_in_rect(&compact, flank_x, grown.y_max - 1.0));
        assert!(point_in_rect(&grown, flank_x, grown.y_max - 1.0));
    }

    #[test]
    fn board_rect_width_matches_the_expanded_formula() {
        let expanded = active_card_rect(Mode::Hud, 0.0, 0.0, 1.0, true, true, false, false);
        let board = board_rect(Mode::Hud, 0.0, 0.0, 1.0, 3, WINDOW_HEIGHT);
        assert_eq!(expanded.x_max - expanded.x_min, board.x_max - board.x_min);
    }

    #[test]
    fn board_rect_one_session_is_the_primary_block_alone() {
        let r = board_rect(Mode::Hud, 0.0, 0.0, 1.0, 1, WINDOW_HEIGHT);
        assert_eq!(r.y_max - r.y_min, HUD_CUTOUT_H + BOARD_PRIMARY_H);
    }

    #[test]
    fn board_rect_grows_by_one_row_height_per_extra_session() {
        let three = board_rect(Mode::Hud, 0.0, 0.0, 1.0, 3, WINDOW_HEIGHT);
        let four = board_rect(Mode::Hud, 0.0, 0.0, 1.0, 4, WINDOW_HEIGHT);
        assert_eq!(
            (four.y_max - four.y_min) - (three.y_max - three.y_min),
            BOARD_ROW_H
        );
    }

    #[test]
    fn board_rect_zero_sessions_matches_one_session_saturating() {
        let zero = board_rect(Mode::Hud, 0.0, 0.0, 1.0, 0, WINDOW_HEIGHT);
        let one = board_rect(Mode::Hud, 0.0, 0.0, 1.0, 1, WINDOW_HEIGHT);
        assert_eq!(zero, one);
    }

    #[test]
    fn board_rect_caps_at_the_window_height_for_many_sessions() {
        let r = board_rect(Mode::Hud, 0.0, 0.0, 1.0, 50, WINDOW_HEIGHT);
        assert_eq!(r.y_max - r.y_min, WINDOW_HEIGHT);
    }

    #[test]
    fn board_rect_caps_at_the_real_window_height_when_taller_than_the_constant() {
        let real_height = 520.0;
        let r = board_rect(Mode::Hud, 0.0, 0.0, 1.0, 3, real_height);
        assert!(r.y_max - r.y_min < real_height);
    }

    #[test]
    fn board_rect_y_flip_uses_the_real_window_height_not_the_stale_constant() {
        let real_height = 520.0;
        let stale = board_rect(Mode::Hud, 0.0, 0.0, 1.0, 1, WINDOW_HEIGHT);
        let fixed = board_rect(Mode::Hud, 0.0, 0.0, 1.0, 1, real_height);
        assert_eq!(stale.y_max, WINDOW_HEIGHT, "sanity: the stale rect's top");
        assert_eq!(
            fixed.y_max, real_height,
            "the real-height rect's top must track the applied window height"
        );
        assert!(
            !point_in_rect(&fixed, WINDOW_WIDTH / 2.0, WINDOW_HEIGHT),
            "a point at the canvas-height window's top must not register as hovered \
             against the real, taller window's rect"
        );
        assert!(point_in_rect(&fixed, WINDOW_WIDTH / 2.0, real_height - 1.0));
    }

    #[test]
    fn board_rect_a_point_far_below_the_real_card_is_correctly_excluded() {
        let real_height = 520.0;
        let r = board_rect(Mode::Hud, 0.0, 0.0, 1.0, 1, real_height);
        let old_rect_midpoint_y = (WINDOW_HEIGHT - HUD_CUTOUT_H - BOARD_PRIMARY_H) / 2.0;
        assert!(
            old_rect_midpoint_y < r.y_min,
            "the chosen probe point must actually be below the fixed rect \
             for this test to prove anything"
        );
        assert!(!point_in_rect(&r, WINDOW_WIDTH / 2.0, old_rect_midpoint_y));
    }

    #[test]
    fn icon_strip_rects_returns_empty_for_zero_present() {
        assert_eq!(
            icon_strip_rects(Mode::Hud, 0.0, 0.0, 1.0, 0, WINDOW_HEIGHT),
            Vec::new()
        );
    }

    #[test]
    fn icon_strip_rects_returns_present_count_entries() {
        for n in 1..=5 {
            assert_eq!(
                icon_strip_rects(Mode::Hud, 0.0, 0.0, 1.0, n, WINDOW_HEIGHT).len(),
                n
            );
        }
    }

    #[test]
    fn icon_strip_rects_two_icons_uses_the_85px_rail_floor() {
        let rects = icon_strip_rects(Mode::Hud, 0.0, 0.0, 1.0, 2, WINDOW_HEIGHT);
        let flank_w = hovered_right_flank_width(2, 1.0);
        assert_eq!(flank_w, FLANK_IDLE); // 85.0, the floor, not the narrower strip_w
        let total_width = HUD_CUTOUT_W + 2.0 * flank_w;
        assert_eq!(total_width, 370.0);
        let card_x_min = (WINDOW_WIDTH - total_width) / 2.0;
        let card_x_max = card_x_min + total_width;
        assert_eq!(rects[1].x_max, card_x_max - FLANK_INSET);
    }

    #[test]
    fn icon_strip_rects_three_icons_matches_the_388px_worked_example() {
        let flank_w = hovered_right_flank_width(3, 1.0);
        assert_eq!(flank_w, 94.0);
        let total_width = HUD_CUTOUT_W + 2.0 * flank_w;
        assert_eq!(total_width, 388.0);
    }

    #[test]
    fn hovered_right_flank_width_pins_the_whole_icon_count_curve() {
        let expected = [85.0, 85.0, 94.0, 120.0, 146.0];
        for (i, want) in expected.iter().enumerate() {
            let n = i + 1;
            assert_eq!(
                hovered_right_flank_width(n, 1.0),
                *want,
                "flank width for {n} present icon(s)"
            );
        }
    }

    #[test]
    fn icon_strip_rects_are_in_left_to_right_order_with_no_overlap_and_correct_gaps() {
        let rects = icon_strip_rects(Mode::Hud, 0.0, 0.0, 1.0, 5, WINDOW_HEIGHT);
        for pair in rects.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            assert!(
                left.x_max <= right.x_min,
                "icons must never overlap: {left:?} vs {right:?}"
            );
            assert_eq!(right.x_min - left.x_max, ICON_GAP);
            assert_eq!(left.x_max - left.x_min, ICON_BOX);
        }
    }

    #[test]
    fn icon_strip_rects_y_span_is_the_cutout_height_alone() {
        let rects = icon_strip_rects(Mode::Hud, 0.0, 0.0, 1.0, 1, WINDOW_HEIGHT);
        assert_eq!(rects[0].y_max - rects[0].y_min, HUD_CUTOUT_H);
        assert_eq!(rects[0].y_max, WINDOW_HEIGHT);
    }

    #[test]
    fn icon_strip_rects_notch_mode_uses_the_real_measured_cutout() {
        let rects_notch = icon_strip_rects(Mode::Notch, 260.0, 34.0, 1.0, 2, WINDOW_HEIGHT);
        let rects_hud = icon_strip_rects(Mode::Hud, 260.0, 34.0, 1.0, 2, WINDOW_HEIGHT);
        assert_ne!(rects_notch[0].x_min, rects_hud[0].x_min);
        assert_eq!(rects_notch[0].y_max - rects_notch[0].y_min, 34.0);
    }

    #[test]
    fn icon_strip_rects_scale_only_affects_the_rail_floor_not_the_raw_icon_geometry() {
        let rects = icon_strip_rects(Mode::Hud, 0.0, 0.0, 1.25, 5, WINDOW_HEIGHT);
        assert_eq!(rects[0].x_max - rects[0].x_min, ICON_BOX);
    }
}
