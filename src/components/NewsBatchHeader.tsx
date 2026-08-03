// Plan 171 (tab-notch redesign, slice I): the news tab's batch header —
// "N fresh · cycle ended Xm ago" plus prev/next nav, mounted above the
// existing shipped news card content inside `NewsBelowBlock.tsx`. New
// because a tab-summoned news card is not a promotion (spec section 7's
// news bullet, `docs/superpowers/specs/2026-08-02-tab-notch-design.md`):
// nothing pushed it, the operator asked for it, so it has to say how big
// the pile is and let them walk it. Set in the masthead's own mono type
// (`news-category.css`'s `.batch-head` rule) so it reads as card chrome,
// not a toolbar — the spec's own wording for this exact line.
//
// Plan 171's pre-implementation sketch included a bespoke `.batch-dots`
// span. The shipped component deliberately omits it: spec section 8
// ("The session bar /
// floor-strip position indicator") explicitly overrides the mock here —
// "News's version needs a plan-time decision the mock's own markup
// leaves ambiguous... Default: implement news's floor strip identically
// to agent's" — meaning the position indicator is the SHARED
// `PositionBar` component (`src/components/PositionBar.tsx`, built in
// Slice F specifically so this slice wouldn't invent a second one), not
// a bespoke dot-strip built inside this header. `PositionBar` mounts as
// `NewsBelowBlock`'s own separate sibling element (absolutely positioned
// to the card floor via the shared `.ttl-bar` CSS), not nested in here.
// This component renders ONLY the text row + nav buttons.
//
// `onPrevious`/`onNext` are presentational callbacks only, same "click
// routing deferred" posture every other new interactive surface in this
// plan follows (see `AgentBelowBlock.tsx`'s `cycleSessionIndex`,
// `IconStrip.tsx`'s `onSelect`). The click-detection MECHANISM is settled
// and shipped — a rust-side `NSEvent` local monitor
// (`src-tauri/src/click.rs`, `docs/ARCHITECTURE.md` §22) observes clicks
// and pushes typed events down the receive-only channel. What is still
// missing is click ROUTING for these specific nav buttons (an open item
// tracked in `plans/README.md`), and the prefix keymap's own
// `[`/`]`-equivalent for news. This component never calls `invoke()` and
// knows nothing about how a click or prefix action actually reaches it.
export function NewsBatchHeader({
  freshCount,
  cycleEndedAgo,
  onPrevious,
  onNext,
}: {
  freshCount: number;
  /** Pre-formatted relative time (e.g. "2m", rendered as "cycle ended 2m
   * ago") — this component does no date-math itself, same "arrives as a
   * plain primitive" discipline `AgentBelowBlock.tsx`'s own doc comment
   * establishes for its props. Nullable (a deliberate widening beyond the
   * plan's own "e.g. cycleEndedAgo: string" sketch): `newsAge`/
   * `newsCategory` are already nullable everywhere else this codebase
   * renders relative-time chrome (`NotificationBody.tsx`'s news branch),
   * and a caller genuinely may have no ended-cycle timestamp yet (e.g.
   * right after startup, before the first poll cycle boundary). `null`
   * omits the "· cycle ended … ago" segment entirely rather than
   * rendering "cycle ended null ago". */
  cycleEndedAgo: string | null;
  onPrevious?: () => void;
  onNext?: () => void;
}) {
  return (
    <div className="batch-head">
      <span>{freshCount} fresh</span>
      {cycleEndedAgo !== null && (
        <>
          <span className="sep">·</span>
          <span>cycle ended {cycleEndedAgo} ago</span>
        </>
      )}
      <span className="batch-nav">
        <button type="button" aria-label="previous story" onClick={onPrevious}>
          ‹
        </button>
        <button type="button" aria-label="next story" onClick={onNext}>
          ›
        </button>
      </span>
    </div>
  );
}
