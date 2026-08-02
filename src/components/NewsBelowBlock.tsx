// Plan 171 (tab-notch redesign, slice I): the news tab's below-block —
// mounted by whichever parent owns the icon-strip's hover-with-a-
// selection shell (Slice K's integration; this component only renders
// what goes INSIDE `.below-block`, the same scope every other below-
// block slice in this plan keeps to — see `AgentBelowBlock.tsx`'s own
// header comment for the identical framing).
//
// Composes three pieces per spec section 7's news bullet
// (`docs/superpowers/specs/2026-08-02-tab-notch-design.md`):
//   1. `NewsBatchHeader` — "N fresh · cycle ended Xm ago" + prev/next.
//   2. the shipped news card content (masthead+category dot, real `Wire`
//      Stamp, headline, category chip, relative age, over the existing
//      `.news-shade`) — REBUILT here rather than imported; see the note
//      below for why.
//   3. `PositionBar` (Slice F, shared per spec section 8's own explicit
//      default: "implement news's floor strip identically to agent's" —
//      see `NewsBatchHeader.tsx`'s header comment for the full citation
//      of why this is NOT a second bespoke dot-strip).
// Plus the existing, already-shipped `<Manifest>` disclosure (spec
// section 7: "prefix+enter opens the existing summary manifest" — not
// new work, just reused as-is).
//
// **Why the news card content is rebuilt here instead of imported from
// `NotificationBody.tsx`** (the extraction this plan's own instructions
// ask to attempt first, matching Slice F's `agentHeroPropsFor` and Slice
// G's `ScoreBlockContent` precedent): unlike `AgentHeroCard`/
// `FootballHeroCard`/`ScoreBlockContent`, the shipped news layout is NOT
// its own exported function — it is inline JSX inside
// `NotificationBody()`'s own `news ? ... : ...` ternary, interleaved with
// the generic branch's markup in the same `.compact`/`.copy` wrapper, and
// paired with `.compact-hint`/`<TtlBar>` chrome this below-block does not
// want (a pulled card never counts down — spec section 9 — and this
// below-block has its own `NewsBatchHeader`/`PositionBar` where the
// shipped promoted card has neither). Extracting the news ternary arm
// into its own exported function the way Slice F/G did would mean
// reshaping `NotificationBody.tsx`'s shared wrapper (also read by the
// generic branch) — a real refactor of a file this slice does not own,
// not a clean pull. Per this plan's own "if the news branch isn't
// cleanly extractable without a large refactor, don't force it" guidance,
// this component instead re-renders the IDENTICAL markup/CSS classes
// (`masthead-row`/`masthead`/`.dot`/`<Stamp>`/`.title.headline`/
// `.notif-meta-row`/`.chip-category`/`.notif-time-inline`, verified
// against `NotificationBody.tsx`'s news branch line-for-line at the time
// of writing) rather than inventing a new look, so the two stay visually
// identical without a shared function backing them. If a future slice
// wants to collapse this into one shared implementation, the news
// ternary arm in `NotificationBody.tsx` is the piece to extract —
// flagged here, not done silently.
//
// **Deliberately no `.compact-hint` ("⌃⇧N more") node**, even though the
// mock's own markup for this page includes one
// (`prototypes/tab-notch-panel.html` ~line 1131): spec section 9 states
// plainly that `prefix enter`/`o` is "the ONLY expansion gesture that
// exists anywhere in this feature" for a tab-summoned card — `⌃⇧N` is a
// different, unrelated shortcut that stays wired to the shipped
// promoted-card flow (spec section 9's "seven shipped combos... completely
// unchanged"). Showing a "⌃⇧N more" hint on THIS card would tell the
// operator the wrong key. The correct hint text depends on the
// operator-configurable prefix keybinding (Settings, Slice J), which this
// presentational component has no way to know — so rather than guess at
// wording or hardcode a hint that might not match the operator's actual
// prefix, this component omits the hint entirely. Conservative choice,
// flagged rather than guessed; a future slice with access to the
// configured prefix string can add a correctly-worded hint here.
//
// **"Visited clears the charge"** (this plan's own Slice I text): once a
// real caller exists (Slice K's integration, or a dedicated follow-up),
// the operator hovering with news selected — i.e. this component actually
// mounting and being seen — should call the rust-side
// `NewsCharge::visit()` (`src-tauri/src/news_charge.rs`) so the charge/
// fresh-count resets for the next cycle, mirroring how selecting/viewing
// a source is the "acknowledgement" gesture spec sections 7/8 describe.
// That requires a real IPC path this slice does not have — `NewsCharge`
// is not wired to `StatusState`/the wire at all yet (Slice B's own
// "landed so far" note: "Not wired into `rss_poller.rs`'s poll loop or
// into `StatusState`... this module is ready to be driven by whichever
// one ends up calling it") — and a real click-detection mechanism (Slice
// A's still-open Mac-Mini hand-off) to fire it from. It is deliberately
// NOT implemented here: no `invoke()`, no fake local "mark visited" state
// that would just be a second, drifting copy of the eventual real one.
// This paragraph is the intended flow, not a stand-in mechanism.
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
