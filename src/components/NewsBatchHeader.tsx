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
// Ported from `prototypes/tab-notch-panel.html`'s own `.batch-head`
// markup (`data-page="news"`, ~line 1107-1118) MINUS that markup's own
// `.batch-dots` span. **This is a deliberate, spec-mandated deviation
// from the mock, not an oversight**: spec section 8 ("The session bar /
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
// `IconStrip.tsx`'s `onSelect`): real click detection is Slice A's still-
// open Mac-Mini hand-off (`plans/171-tab-notch-redesign.md`, Slice A item
// 2), and the prefix keymap's own `[`/`]`-equivalent wiring for news is
// Slice D's/Slice K's. This component never calls `invoke()` and knows
// nothing about how a click or prefix action actually reaches it.
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
