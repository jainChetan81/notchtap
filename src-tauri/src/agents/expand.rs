//! Pure geometry for the Agent Board's hover-EXPANDED window frame.
//!
//! The one deliberate exception to `hover::active_card_rect`'s "the
//! window frame never changes; only the CSS width within it does":
//! [`expanded_board_frame`] computes a REAL window frame (`lib.rs`'s
//! hover-transition call site applies it via `set_size`/`set_position`),
//! because a screen-bounded scrolling Board cannot fit the fixed
//! `hover::WINDOW_HEIGHT` canvas. Width stays pinned to
//! `hover::BASE_EXPANDED`; only height and vertical position change.
//! No AppKit types here — plain function over already-fetched numbers;
//! the `NSScreen`/`NSWindow` calls live only in `lib.rs`.

/// Duplicated-constants pair with `hover::BASE_EXPANDED` — any change
/// to one MUST change the other in the same commit.
pub const EXPANDED_BOARD_WIDTH: f64 = 500.0;

/// Duplicated-constants pair with `hover::WINDOW_HEIGHT` — the resting
/// frame's fixed height, and the floor the expanded height formula
/// never drops below.
pub const RESTING_WINDOW_HEIGHT: f64 = 300.0;

/// Conservative budget for everything the expanded Board draws ABOVE
/// its scrollable session list: shell flank/cutout row, the primary
/// session's HERO card (`AgentBoard.tsx`), and the list's top margin.
/// Lockstep pair with `agent-board.css`'s
/// `.agent-board-expanded-scroll { max-height: calc(100vh - 210px) }` —
/// any change to one MUST change the other in the same commit.
const HEADER_HEIGHT: f64 = 210.0;

/// Conservative per-row budget for one `ExpandedAgentRow`
/// (`AgentBoard.tsx` / `agent-board.css`) plus the between-row gap; if
/// that template gains or loses a line, this number must move in the
/// same commit. Budgets the COMMON row shape only — optional blocks
/// (detail pairs, history disclosure) are unbounded and scroll via the
/// `.agent-board-expanded-scroll` cap instead; sizing for the maximum
/// possible row would oversize every ordinary hover. Never reach for a
/// resting-row number in this frame math.
const EXPANDED_ROW_HEIGHT: f64 = 96.0;

/// Never claim more than this fraction of the screen's height, however
/// many sessions are retained.
const MAX_SCREEN_FRACTION: f64 = 0.75;

/// A window frame in the same screen-space, top-left-origin convention
/// as `lib.rs::position_top_center` — NOT `hover::Rect`'s AppKit
/// bottom-left convention, which describes a region WITHIN a fixed
/// window rather than the window itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardWindowFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The expanded Board's window frame for a `session_count`-session
/// board on a `screen_width` x `screen_height` monitor (logical points,
/// same units `current_monitor()` reports post `to_logical`).
///
/// - width: `EXPANDED_BOARD_WIDTH`, capped at `screen_width` so a
///   narrow screen never produces an off-screen window;
/// - height: `HEADER_HEIGHT + EXPANDED_ROW_HEIGHT * (session_count - 1)`
///   — the primary session lives in the hero block, so only the OTHER
///   sessions are rows (`AgentBoard.tsx` renders `sessions[1..]`) —
///   floored at `RESTING_WINDOW_HEIGHT`, capped at
///   `screen_height * MAX_SCREEN_FRACTION`; content beyond that
///   scrolls in the frontend's bounded container;
/// - horizontally centered; anchored FLUSH at the screen's top edge
///   (`y = 0`) like `lib.rs::position_window` — any top margin makes
///   the shell visibly detach on hover and re-attach on leave.
pub fn expanded_board_frame(
    screen_width: f64,
    screen_height: f64,
    session_count: usize,
) -> BoardWindowFrame {
    let row_count = session_count.saturating_sub(1);
    let content_height = HEADER_HEIGHT + EXPANDED_ROW_HEIGHT * row_count as f64;
    let max_height = (screen_height * MAX_SCREEN_FRACTION).max(RESTING_WINDOW_HEIGHT);
    let height = content_height.max(RESTING_WINDOW_HEIGHT).min(max_height);
    let width = EXPANDED_BOARD_WIDTH.min(screen_width.max(0.0));
    let x = ((screen_width - width) / 2.0).max(0.0);
    BoardWindowFrame {
        x,
        y: 0.0,
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
        // 3 and 5 specifically: lower counts hit the resting-height
        // floor, 7+ hits the screen-fraction cap — either would stop
        // this pair from exercising the linear-growth formula.
        let three = expanded_board_frame(SCREEN_W, SCREEN_H, 3);
        let five = expanded_board_frame(SCREEN_W, SCREEN_H, 5);
        assert_eq!(five.height - three.height, EXPANDED_ROW_HEIGHT * 2.0);
    }

    #[test]
    fn the_primary_session_is_the_hero_not_a_row_so_only_the_rest_are_counted() {
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 6);
        assert_eq!(frame.height, HEADER_HEIGHT + EXPANDED_ROW_HEIGHT * 5.0);
    }

    #[test]
    fn three_sessions_budget_a_full_expanded_row_each_never_a_clipped_slot() {
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 3);
        assert_eq!(frame.height, HEADER_HEIGHT + EXPANDED_ROW_HEIGHT * 2.0);
        // Guard the CONSTANT itself, independent of the formula equality
        // above: the budget must cover a real ExpandedAgentRow's ~90px
        // name/summary/meta stack. Const block fails at compile time.
        const {
            assert!(
                EXPANDED_ROW_HEIGHT >= 90.0,
                "EXPANDED_ROW_HEIGHT must cover a real expanded row"
            );
        }
    }

    #[test]
    fn many_sessions_caps_at_the_screen_fraction_not_content_height() {
        // 30 sessions want ~2994px of content — far over the cap.
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 30);
        let uncapped_content = HEADER_HEIGHT + EXPANDED_ROW_HEIGHT * 29.0;
        assert!(frame.height < uncapped_content);
        assert_eq!(frame.height, SCREEN_H * MAX_SCREEN_FRACTION);
    }

    #[test]
    fn eight_sessions_hit_the_screen_cap_and_scroll() {
        // The cap binds before 8 sessions on a 982pt screen; the frame
        // stays screen-bounded and the surplus scrolls in the frontend's
        // bounded container — capped is the cap doing its job.
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 8);
        assert_eq!(frame.height, SCREEN_H * MAX_SCREEN_FRACTION);
        assert!(frame.height < HEADER_HEIGHT + EXPANDED_ROW_HEIGHT * 7.0);
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
    fn anchored_flush_at_the_screen_top_edge_exactly_like_the_resting_frame() {
        // Any nonzero y makes the shell visibly drop away from the
        // screen top on hover-expand and snap back on leave; the resting
        // frame sits at y = 0 (`lib.rs::position_window`).
        for session_count in [0, 1, 4, 30] {
            let frame = expanded_board_frame(SCREEN_W, SCREEN_H, session_count);
            assert_eq!(frame.y, 0.0);
        }
    }

    #[test]
    fn named_constants_match_hovers_own_duplicated_constants() {
        // Tripwire for the duplicated-constant pairs in `hover.rs` — if
        // either number moves, check both sides.
        assert_eq!(EXPANDED_BOARD_WIDTH, 500.0);
        assert_eq!(RESTING_WINDOW_HEIGHT, 300.0);
    }
}
