// News tab's below-block, mounted by whichever parent owns the icon-strip's
// hover-with-a-selection shell. This component only renders what goes INSIDE
// `.below-block`, the same scope as `AgentBelowBlock.tsx`.
//
// Composes three pieces per the tab-notch design:
//   1. `NewsBatchHeader` — "N fresh · cycle ended Xm ago" + prev/next.
//   2. the shipped news card content (masthead+category dot, real `Wire`
//      Stamp, headline, category chip, relative age, over `.news-shade`) —
//      REBUILT here rather than imported (see below for why).
//   3. `PositionBar` (shared: news's floor strip is implemented identically
//      to agent's — see `NewsBatchHeader.tsx`).
// Plus the already-shipped `<Manifest>` disclosure (prefix+enter opens the
// existing summary manifest — reused as-is).
//
// Why the news card content is rebuilt here instead of imported from
// `NotificationBody.tsx` (the extraction tried first, matching
// `agentHeroPropsFor` and `ScoreBlockContent`): unlike
// `AgentHeroCard`/`FootballHeroCard`, the shipped news layout is NOT its own
// exported function — it is inline JSX inside `NotificationBody()`'s
// `news ? ... : ...` ternary, interleaved with the generic branch in the
// same `.compact`/`.copy` wrapper, and paired with
// `.compact-hint`/`<TtlBar>` chrome this below-block does not want (a pulled
// card never counts down, and it has its own
// `NewsBatchHeader`/`PositionBar`). Extracting the news arm would reshape
// `NotificationBody.tsx`'s shared wrapper (also read by the generic branch)
// — a real refactor of a file this component does not own. It instead
// re-renders the IDENTICAL markup/CSS classes (`masthead-row`/`masthead`/
// `.dot`/`<Stamp>`/`.title.headline`/`.notif-meta-row`/`.chip-category`/
// `.notif-time-inline`) so the two stay visually identical without a shared
// function. If a future slice wants one shared implementation, the news
// ternary arm in `NotificationBody.tsx` is the piece to extract.
//
// Deliberately no `.compact-hint` ("⌃⇧N more") node: `prefix enter`/`o`
// is the ONLY expansion gesture for a tab-summoned card — `⌃⇧N` is a
// different, unrelated shortcut. Showing a "⌃⇧N more" hint here would
// tell the operator the wrong key, and the correct hint text depends on
// the operator-configurable prefix keybinding this presentational
// component has no way to know (Settings). Omits the hint rather than
// guess wording.
//
// "Visited clears the charge": once a real caller exists, the operator
// hovering with news selected — this component mounting and being seen —
// should call the rust-side `NewsCharge::visit()`
// (`src-tauri/src/news_charge.rs`) so the charge/fresh-count resets for
// the next cycle, mirroring how viewing a source is the "acknowledgement"
// gesture. That needs a real IPC + click-detection path that does not
// exist here. Deliberately NOT implemented: no `invoke()`, no fake local
// "mark visited" state that would be a second, drifting copy of the real
// one. This is the intended flow, not a stand-in mechanism.
import type { Priority } from "../lib/presentation";
import { categoryClass, categoryLabel } from "../lib/presentation";
import { Manifest } from "./Manifest";
import { NewsBatchHeader } from "./NewsBatchHeader";
import { PositionBar } from "./PositionBar";
import { Stamp } from "./Stamp";

// One story in the current cycle — every field arrives pre-computed as a
// plain primitive, same "no wire-specific type import" discipline
// `AgentBelowBlock.tsx`'s own doc comment establishes for its props (this
// component needs no `SlotState`/`EspnMeta` import).
//
// **Flagged, not improvised** (matching `FootballHeroCard`'s own
// `secondaryMatches` doc in `NotificationBody.tsx`): nothing on the wire
// currently supplies this shape. `StatusState.news` (`src-tauri/src/
// status.rs`) is still just `{ enabled: bool }` (confirmed by reading it
// directly), and `NewsCharge` (`src-tauri/src/news_charge.rs`, Slice B) is
// a pure state machine with no caller wiring it into `rss_poller.rs`'s
// poll loop or onto the wire at all yet. Extending that wire shape is
// explicitly Slice A's call per the plan's own section 0 cross-slice
// contract ("Icon presence/liveness... Slice A owns defining this"), and
// out of this slice's file scope (`src-tauri/src/status.rs`/
// `news_charge.rs` are both off-limits here per this slice's own
// boundaries). This component is built to be correct once a real caller
// supplies these fields; until then nothing populates it.
export interface NewsStoryView {
  /** Masthead label, e.g. "The Verge" — already resolved (the shipped
   * card's own `slot.source ?? "RSS"` fallback, `NotificationBody.tsx`,
   * is the caller's job to apply, not this component's). */
  source: string;
  headline: string;
  /** The manifest's summary body — `Manifest`'s own `body` prop, opened
   * via `prefix+enter` (spec section 7's news bullet). */
  summary: string;
  /** Raw category key (e.g. "tech"), fed through the same
   * `categoryClass`/`categoryLabel` helpers `StatusRailCard.tsx` already
   * uses for the shipped single-item news card — `null` renders no chip
   * and keys the below-block's own class to the generic/neutral
   * category, same as today. */
  category: string | null;
  /** Pre-formatted relative age (e.g. "12m"), or `null` to omit the
   * segment — same convention `newsAge` already follows in
   * `NotificationBody.tsx`. */
  age: string | null;
  priority: Priority;
  /** Whether "read the story" (the shipped card's `⌃⇧O`) has a real link
   * to open — feeds `Manifest`'s own `hasLink` prop (its footer hint text
   * differs with/without one). */
  hasLink: boolean;
}

export function NewsBelowBlock({
  stories,
  currentIndex,
  freshCount,
  cycleEndedAgo,
  expanded,
  onPrevious,
  onNext,
}: {
  stories: NewsStoryView[];
  /** Not itself clamped by the caller — this component defends against
   * an out-of-range index the same way `AgentBelowBlock.tsx`'s own
   * `viewedIndex` does, rather than trusting every caller to re-derive a
   * valid index before every render. */
  currentIndex: number;
  freshCount: number;
  cycleEndedAgo: string | null;
  /** Whether the summary `<Manifest>` panel is open — `prefix+enter`/`o`
   * (spec section 9: "the ONLY expansion gesture that exists anywhere in
   * this feature"). Required, not defaulted, matching
   * `NotificationBody.tsx`'s own `expanded` prop — an explicit caller
   * decision, never an implicit default that could silently diverge from
   * the shell's real expand state. */
  expanded: boolean;
  onPrevious?: () => void;
  onNext?: () => void;
}) {
  // Same "nothing to show, render nothing" posture `AgentBelowBlock.tsx`'s
  // own `sessions.length === 0` guard uses, and `PositionBar`'s own
  // `total <= 0` guard mirrors independently.
  if (stories.length === 0) {
    return null;
  }
  const clampedIndex = Math.min(Math.max(currentIndex, 0), stories.length - 1);
  const story = stories[clampedIndex];

  return (
    <div
      className={`below-block news-shade ${categoryClass(story.category)}`}
      data-testid="news-below-block"
    >
      <NewsBatchHeader
        freshCount={freshCount}
        cycleEndedAgo={cycleEndedAgo}
        onPrevious={onPrevious}
        onNext={onNext}
      />
      {/* Verbatim against `NotificationBody.tsx`'s news branch (masthead-
          row/masthead/dot/Stamp/title.headline/notif-meta-row/
          chip-category/notif-time-inline) — see this file's header
          comment for why this is a re-render rather than an import. */}
      <div className="compact">
        <div className="copy">
          <div className="masthead-row">
            <div className="masthead">
              <span className="dot" />
              {story.source}
            </div>
            <Stamp priority={story.priority} signal="generic" eventType="news_item" />
          </div>
          <div className="title headline">{story.headline}</div>
          {(story.category !== null || story.age !== null) && (
            <div className="notif-meta-row">
              {story.category !== null && (
                <span className="chip chip-category">{categoryLabel(story.category)}</span>
              )}
              {story.age !== null && <span className="notif-time-inline">{story.age}</span>}
            </div>
          )}
        </div>
      </div>
      <Manifest
        title={story.headline}
        body={story.summary}
        eventType="news_item"
        expanded={expanded}
        hasLink={story.hasLink}
      />
      <PositionBar total={stories.length} current={clampedIndex} />
    </div>
  );
}
