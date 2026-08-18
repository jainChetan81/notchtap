// The news tab's batch header — "N fresh · cycle ended Xm ago" plus
// prev/next nav, mounted above the shipped news card content inside
// `NewsBelowBlock.tsx`. New because a tab-summoned news card is not a
// promotion: nothing pushed it, the operator asked for it, so it has to
// say how big the pile is and let them walk it. Set in the masthead's own
// mono type (`news-category.css`'s `.batch-head` rule) so it reads as card
// chrome, not a toolbar.
//
// Deliberately no bespoke `.batch-dots` span: the position indicator is
// the SHARED `PositionBar` component (`src/components/PositionBar.tsx`,
// news's floor strip implemented identically to agent's), which mounts as
// `NewsBelowBlock`'s separate sibling (absolutely positioned to the card
// floor via the shared `.ttl-bar` CSS), not nested in here. This component
// renders ONLY the text row + nav buttons.
//
// `onPrevious`/`onNext` are presentational callbacks only, same "click
// routing deferred" posture the other interactive surfaces hold (see
// `AgentBelowBlock.tsx`'s `cycleSessionIndex`, `IconStrip.tsx`'s
// `onSelect`). Clicks are detected rust-side by an `NSEvent` local
// monitor (`src-tauri/src/click.rs`, `docs/ARCHITECTURE.md` §22) and
// pushed as typed events down the receive-only channel; routing those
// events to these specific nav buttons, and the prefix keymap's own
// `[`/`]`-equivalent for news, are open items. This component never
// calls `invoke()` and knows nothing about how a click or prefix action
// actually reaches it.
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
   * establishes for its props. Nullable: `newsAge`/
   * `newsCategory` are nullable everywhere else this codebase
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
