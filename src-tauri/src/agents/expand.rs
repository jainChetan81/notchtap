//! Plan 142 (v7 ticket 10 of 13, `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md`
//! §6.2 expanded): pure geometry for the Agent Board's hover-EXPANDED
//! window frame.
//!
//! `hover::active_card_rect`'s own doc states its governing invariant
//! plainly: "the window frame never changes; only the CSS width within
//! it does." That invariant holds for every OTHER card shape in this
//! app, but the expanded Board deliberately breaks it — spec §6.2's own
//! words are "screen-bounded maximum height and scrolling," which the
//! fixed 300px canvas (`hover::WINDOW_HEIGHT`) cannot satisfy once
//! enough sessions are retained. This module is the one, deliberate,
//! documented exception: [`expanded_board_frame`] computes a REAL
//! window frame (`lib.rs`'s hover-transition call site does the actual
//! `set_size`/`set_position`), not a CSS width.
//!
//! Width stays pinned to `hover::BASE_EXPANDED` (the same 500px design
//! width the RESTING board already renders at via its permanent
//! `.card-assembly.expanded` class, `AgentBoard.tsx`) — only the height
//! and vertical position change between the resting frame and the
//! expanded one. No AppKit types anywhere here, mirroring
//! `presentation::presentation_mode`'s split from its own subprocess
//! caller (`docs/TESTING_STRATEGY.md` §4.4): this is a plain function
//! over already-fetched numbers, unit-testable without a GUI; the
//! `NSScreen`/`NSWindow` calls that gather `screen_width`/`screen_height`
//! and apply the result live only in `lib.rs`.

/// Duplicated-constants pair with `hover::BASE_EXPANDED` — see that
/// constant's own doc for the discipline. Any change to one MUST change
/// the other in the same commit.
pub const EXPANDED_BOARD_WIDTH: f64 = 500.0;

/// Duplicated-constants pair with `hover::WINDOW_HEIGHT` — the resting
/// frame's own fixed height, and the floor this module's expanded
/// height formula never drops below (a Board with exactly one session
/// should never look SMALLER expanded than it did resting).
pub const RESTING_WINDOW_HEIGHT: f64 = 300.0;

/// Conservative estimate (same CONSERVATIVE-never-generous discipline
/// `hover.rs`'s own `BELOW_BLOCK_SHOWING_H`/`BELOW_BLOCK_EXPANDED_H`
/// constants document) of the expanded header block above the
/// scrollable session list — the primary card's head/project/summary/
/// elapsed lines, plus the shell's own flank/cutout row.
const HEADER_HEIGHT: f64 = 150.0;

/// Conservative estimate of one expanded row's rendered height
/// (`agent-board.css`'s `.agent-expanded-row`) — taller than the resting
/// board's compact `.agent-row` because the expanded row carries its own
/// per-row disclosure affordance.
const ROW_HEIGHT: f64 = 34.0;

/// Never claim more than this fraction of the screen's height, however
/// many sessions are retained — "screen-bounded," not "however tall the
/// content wants to be."
const MAX_SCREEN_FRACTION: f64 = 0.75;

/// Keep the expanded panel's top edge clear of the literal screen edge
/// (and, in practice, the menu bar it overlaps) rather than flush at
/// `y = 0` the way the resting frame is.
const TOP_MARGIN: f64 = 8.0;

/// A window frame in the same screen-space, top-left-origin convention
/// `lib.rs::position_top_center` already uses for `PhysicalPosition`/
/// `LogicalPosition` (NOT `hover::Rect`'s AppKit bottom-left convention —
/// that type describes a region WITHIN a fixed window; this one
/// describes the window itself).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardWindowFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The expanded Board's window frame for a `session_count`-session board
/// on a `screen_width` x `screen_height` monitor (logical points, same
/// units `current_monitor()` reports post `to_logical`).
///
/// - width: `EXPANDED_BOARD_WIDTH`, capped at `screen_width` (an
///   unrealistically narrow screen must never produce an off-screen
///   window);
/// - height: `HEADER_HEIGHT + ROW_HEIGHT * session_count`, floored at
///   `RESTING_WINDOW_HEIGHT` (never shrink below the resting frame) and
///   capped at `screen_height * MAX_SCREEN_FRACTION` (the screen-bounded
///   maximum spec §6.2 calls for — content beyond that scrolls, per the
///   frontend's own bounded scroll container);
/// - horizontally centered; vertically anchored `TOP_MARGIN` below the
///   screen's top edge.
pub fn expanded_board_frame(
    screen_width: f64,
    screen_height: f64,
    session_count: usize,
) -> BoardWindowFrame {
    let content_height = HEADER_HEIGHT + ROW_HEIGHT * session_count as f64;
    let max_height = (screen_height * MAX_SCREEN_FRACTION).max(RESTING_WINDOW_HEIGHT);
    let height = content_height.max(RESTING_WINDOW_HEIGHT).min(max_height);
    let width = EXPANDED_BOARD_WIDTH.min(screen_width.max(0.0));
    let x = ((screen_width - width) / 2.0).max(0.0);
    BoardWindowFrame {
        x,
        y: TOP_MARGIN,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN_W: f64 = 1512.0;
    const SCREEN_H: f64 = 982.0;

    #[test]
    fn zero_sessions_floors_at_the_resting_window_height() {
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 0);
        assert_eq!(frame.height, RESTING_WINDOW_HEIGHT);
    }

    #[test]
    fn one_session_is_at_least_the_resting_height_never_smaller() {
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 1);
        assert!(frame.height >= RESTING_WINDOW_HEIGHT);
    }

    #[test]
    fn a_handful_of_sessions_under_the_cap_grows_linearly_with_count() {
        // 5 and 7, not e.g. 3 and 5: content_height(3) = 150 + 34*3 = 252,
        // still under the RESTING_WINDOW_HEIGHT (300) floor, so a lower
        // pair would compare two FLOORED (equal) heights instead of
        // exercising the linear-growth formula this test targets.
        let five = expanded_board_frame(SCREEN_W, SCREEN_H, 5);
        let seven = expanded_board_frame(SCREEN_W, SCREEN_H, 7);
        assert_eq!(seven.height - five.height, ROW_HEIGHT * 2.0);
    }

    #[test]
    fn many_sessions_caps_at_the_screen_fraction_not_content_height() {
        // 30 sessions would want HEADER_HEIGHT + 30*ROW_HEIGHT = 1170px —
        // comfortably over 0.75 * 982 = 736.5, so the cap must win.
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 30);
        let uncapped_content = HEADER_HEIGHT + ROW_HEIGHT * 30.0;
        assert!(frame.height < uncapped_content);
        assert_eq!(frame.height, SCREEN_H * MAX_SCREEN_FRACTION);
    }

    #[test]
    fn eight_sessions_the_plans_own_test_floor_stays_comfortably_under_the_cap() {
        // The plan's own manual-check floor ("many (8+) sessions") — prove
        // it lands in the linear regime, not already clipped by the cap,
        // so the frontend's scroll-container test actually exercises real
        // growth rather than a pre-capped constant.
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 8);
        assert_eq!(frame.height, HEADER_HEIGHT + ROW_HEIGHT * 8.0);
        assert!(frame.height < SCREEN_H * MAX_SCREEN_FRACTION);
    }

    #[test]
    fn width_is_the_design_width_on_an_ordinary_screen() {
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 4);
        assert_eq!(frame.width, EXPANDED_BOARD_WIDTH);
    }

    #[test]
    fn width_caps_at_the_screen_width_on_a_narrow_screen() {
        let frame = expanded_board_frame(320.0, SCREEN_H, 4);
        assert_eq!(frame.width, 320.0);
    }

    #[test]
    fn horizontally_centered_on_the_screen() {
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 4);
        assert_eq!(frame.x, (SCREEN_W - frame.width) / 2.0);
    }

    #[test]
    fn anchored_top_margin_below_the_screen_edge_not_flush() {
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 4);
        assert_eq!(frame.y, TOP_MARGIN);
    }

    #[test]
    fn named_constants_match_hovers_own_duplicated_constants() {
        // Tripwire, same discipline as `hover.rs`'s own
        // `active_card_rect_geometry_constants_match_named_style_constants`
        // — a reviewer diffing this file sees the citation and checks both
        // sides if either number ever moves.
        assert_eq!(EXPANDED_BOARD_WIDTH, 500.0);
        assert_eq!(RESTING_WINDOW_HEIGHT, 300.0);
    }
}
