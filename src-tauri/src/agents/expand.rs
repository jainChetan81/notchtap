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
/// constants document) of everything the expanded Board draws ABOVE its
/// scrollable session list: the shell's own flank/cutout row, the
/// primary session's HERO card (`AgentHeroCard`'s masthead/title/
/// subtitle/body/fact-pill template, `AgentBoard.tsx`), and the list's
/// own top margin.
///
/// Operator feedback (2026-08-02): the hero used to be REPLACED by the
/// expanded list, so this constant only had to cover a bare header —
/// hovering a one-session Board swapped its big hero for one skinny
/// row, i.e. hover made the card look smaller. The hero now stays
/// mounted in BOTH states and the list carries only the OTHER sessions,
/// so this height must budget for the hero itself.
///
/// Lockstep pair with `agent-board.css`'s
/// `.agent-board-expanded-scroll { max-height: calc(100vh - 210px) }` —
/// that reserve is this same above-the-list block, measured against the
/// window height this module computes. Any change to one MUST change
/// the other in the same commit.
const HEADER_HEIGHT: f64 = 210.0;

/// Conservative per-row budget for what the hover-EXPANDED Board
/// actually renders below its hero: one `ExpandedAgentRow`
/// (`AgentBoard.tsx`), whose `agent-board.css` template is a multi-line
/// stack — name row, summary line, meta row — plus the list's
/// between-row gap. Lockstep-references that component: if its template
/// gains or loses a line, this number must move in the same commit.
///
/// Operator feedback (2026-08-02): this budget used to be a 34px
/// `ROW_HEIGHT`, which is the COMPACT RESTING row's height
/// (`.agent-row`) and roughly a third of an expanded row's. The resting
/// board is a fixed [`RESTING_WINDOW_HEIGHT`] canvas that no per-row
/// arithmetic drives, so that constant never belonged in this formula
/// at all; with it, a three-session Board sized its window to give each
/// expanded row a ~68px slot and visibly clipped them mid-row. It has
/// been deleted rather than kept — nothing in the frame math is allowed
/// to reach for a resting-row number.
const EXPANDED_ROW_HEIGHT: f64 = 96.0;

/// Never claim more than this fraction of the screen's height, however
/// many sessions are retained — "screen-bounded," not "however tall the
/// content wants to be."
const MAX_SCREEN_FRACTION: f64 = 0.75;

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
/// - height: `HEADER_HEIGHT + EXPANDED_ROW_HEIGHT * (session_count - 1)` — the
///   primary session lives in the HERO block `HEADER_HEIGHT` already
///   budgets for, so only the OTHER sessions are rows (`AgentBoard.tsx`
///   renders exactly `sessions[1..]` as expanded rows) — floored at
///   `RESTING_WINDOW_HEIGHT` (never shrink below the resting frame) and
///   capped at `screen_height * MAX_SCREEN_FRACTION` (the screen-bounded
///   maximum spec §6.2 calls for — content beyond that scrolls, per the
///   frontend's own bounded scroll container);
/// - horizontally centered; anchored FLUSH at the screen's top edge
///   (`y = 0`), exactly like the resting frame `lib.rs::position_window`
///   places. Operator feedback (2026-08-02): an 8px top margin here made
///   the whole shell visibly detach from the top of the screen on hover
///   and re-attach on leave.
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
        // 3 and 5, not e.g. 1 and 2: content_height(2) = 210 + 96 = 306 is
        // only just over the RESTING_WINDOW_HEIGHT (300) floor, and
        // content_height(1) = 210 is under it, so a lower pair would
        // compare FLOORED heights instead of exercising the linear-growth
        // formula this test targets. Not 7+ either: content_height(7) =
        // 210 + 96*6 = 786 is already over the 0.75 * 982 = 736.5 cap.
        let three = expanded_board_frame(SCREEN_W, SCREEN_H, 3);
        let five = expanded_board_frame(SCREEN_W, SCREEN_H, 5);
        assert_eq!(five.height - three.height, EXPANDED_ROW_HEIGHT * 2.0);
    }

    #[test]
    fn the_primary_session_is_the_hero_not_a_row_so_only_the_rest_are_counted() {
        // Operator feedback (2026-08-02): the hero card stays mounted while
        // expanded, and the list below it carries `sessions[1..]` only —
        // so N sessions is a hero plus N-1 rows, never N rows.
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 6);
        assert_eq!(frame.height, HEADER_HEIGHT + EXPANDED_ROW_HEIGHT * 5.0);
    }

    #[test]
    fn three_sessions_budget_a_full_expanded_row_each_never_a_clipped_slot() {
        // Operator screenshot (2026-08-02): the frame used to budget the
        // COMPACT resting row's 34px per row, so a three-session Board gave
        // each `ExpandedAgentRow` a ~68px slot and clipped them mid-row.
        // The rows are ~90px tall; the frame must say so.
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 3);
        assert_eq!(frame.height, HEADER_HEIGHT + EXPANDED_ROW_HEIGHT * 2.0);
        let per_row_slot = (frame.height - HEADER_HEIGHT) / 2.0;
        assert!(
            per_row_slot >= 96.0,
            "each expanded row got {per_row_slot}px, under a real row's height"
        );
    }

    #[test]
    fn many_sessions_caps_at_the_screen_fraction_not_content_height() {
        // 30 sessions would want HEADER_HEIGHT + 29*EXPANDED_ROW_HEIGHT =
        // 2994px — far over 0.75 * 982 = 736.5, so the cap must win.
        let frame = expanded_board_frame(SCREEN_W, SCREEN_H, 30);
        let uncapped_content = HEADER_HEIGHT + EXPANDED_ROW_HEIGHT * 29.0;
        assert!(frame.height < uncapped_content);
        assert_eq!(frame.height, SCREEN_H * MAX_SCREEN_FRACTION);
    }

    #[test]
    fn eight_sessions_the_plans_own_test_floor_hits_the_cap_and_scrolls() {
        // The plan's own manual-check floor ("many (8+) sessions"). With the
        // real ~96px expanded-row budget the cap now binds well before 8 on
        // a 982pt screen (content would be 210 + 96*7 = 882 > 736.5), so the
        // contract this exercises is the OTHER half of spec §6.2: the frame
        // stays screen-bounded and the surplus scrolls inside the
        // frontend's own bounded container. It is not a regression that this
        // count is capped — it is the cap doing its job.
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
        // Operator feedback (2026-08-02): any nonzero y here makes the whole
        // shell visibly drop away from the top of the screen on hover-expand
        // and snap back on leave. The resting frame sits at y = 0
        // (`lib.rs::position_window`); the expanded one must too.
        for session_count in [0, 1, 4, 30] {
            let frame = expanded_board_frame(SCREEN_W, SCREEN_H, session_count);
            assert_eq!(frame.y, 0.0);
        }
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
