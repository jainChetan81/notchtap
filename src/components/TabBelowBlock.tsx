// Selection-driven below-block swap: hovering with a selection mounts the
// shipped card for that source, reused, not reinvented.
//
// This component owns ONLY the routing decision. Every branch below is a
// component another slice already built and tested (`AgentBelowBlock`,
// `NewsBelowBlock`) — nothing here re-renders card markup of its own.
//
// Football is deliberately NOT routed here: `IdleHoverPeek.tsx`'s own
// mechanism stays untouched, and selection reaches that shipped component
// via its `prefer` prop instead (see `StatusRailCard.tsx`'s mount site and
// `PeekPreference`'s doc). A second copy of the card here would be exactly
// the drift "reuse what's shipped" exists to prevent.
//
// "Nothing selected" is not a special case: `selected === null` returns
// `null`, so with no `.below-block` in the DOM, `card-chrome.css`'s
// `:not(:has(.below-block))` rounding law hands the outer corners back to
// the flanks by itself.
import type { AgentSessionView } from "../useAgentState";
import type { StatusState } from "../useStatusState";
import { AgentBelowBlock } from "./AgentBelowBlock";
import type { Tab } from "./IconStrip";
import { NewsBelowBlock, type NewsStoryView } from "./NewsBelowBlock";

/// The tabs this component actually renders. Football is handled by
/// `IdleHoverPeek`'s `prefer` prop instead (see the header note) —
/// expressed in the type so a caller can't route it here by accident and
/// silently get the "none" page.
export type TabBelowBlockTab = Extract<Tab, "agent" | "news">;

export function tabBelowBlockHandles(selected: Tab | null): selected is TabBelowBlockTab {
  return selected === "agent" || selected === "news";
}

// No wire source exists for news story CONTENT. `StatusState.news`
// carries the charge cycle only (`chargeFraction`/`chargeCount`/
// `isCharged` — slice B's `NewsCharge`, wired through in slice A), never
// the stories themselves; `NewsBelowBlock`'s own `NewsStoryView` doc
// already flags this as "built to be correct once a real caller supplies
// these fields; until then nothing populates it". So the graceful
// minimum its own first test pins ("renders nothing when there are zero
// stories") is what mounts today: the batch charge is visible on the
// news GLYPH (`newsCharge`/`newsCount`, which do have a wire source),
// and the card itself appears the moment a story wire lands, with no
// change to this file. Deliberately not faked with placeholder stories.
const NO_NEWS_STORIES: NewsStoryView[] = [];

export function TabBelowBlock({
  selected,
  status,
  agentSessions,
  agentCapturedAtMs,
  nowMs = Date.now(),
  viewedSessionIndex = 0,
  expanded = false,
}: {
  selected: Tab | null;
  /// Optional for the same reason `StatusRailCard`'s own `status` prop
  /// is — the settings-window preview and most component tests render
  /// that card with no status wire at all. A missing wire simply means
  /// a zero fresh-count.
  status: StatusState | undefined;
  agentSessions: AgentSessionView[];
  agentCapturedAtMs: number;
  /// Defaulted rather than required so the shipped mount site doesn't
  /// have to thread a clock it has no other use for — the same
  /// `Date.now()`-at-render idiom `StatusRailCard`'s own `ageLabel` call
  /// already uses. Tests pass it explicitly to pin elapsed labels
  /// without spying on the global, matching `agentHeroPropsFor`'s own
  /// `nowMs` parameter (AgentBoard.tsx).
  nowMs?: number;
  /// `prefix-[`/`prefix-]` cycles this (spec section 9), and
  /// `AgentBelowBlock.cycleSessionIndex` is the pure wraparound that
  /// moves it. Live end to end as of plan 184: rust owns the cursor and
  /// emits `agent-viewed-session-changed` (written by both the prefix
  /// follow-ups and the auto-advance timer), `useAgentViewedSession`
  /// listens for it in App.tsx, and `StatusRailCard` threads the value
  /// down to here. Still optional so callers with no wire at all (tests,
  /// the settings preview) render against the first session.
  viewedSessionIndex?: number;
  /// `prefix+enter`/`o` (spec section 9's only expansion gesture). Unlike
  /// `viewedSessionIndex` above, nothing threads this one during a pull —
  /// the prefix's ExpandToggle routes to `toggle_manual_expand`, which
  /// drives the notification card's own manual expand, not this prop — so
  /// it defaults to the compact form, which spec section 2 decision 6
  /// makes the correct default regardless ("hover always shows the
  /// selected tab's card in COMPACT form. Never auto-expands").
  expanded?: boolean;
}) {
  if (!tabBelowBlockHandles(selected)) {
    return null;
  }

  switch (selected) {
    case "agent":
      // `AgentBelowBlock` renders nothing for an empty session list, so a
      // selection whose source has gone quiet degrades to the "none" page
      // on its own — spec section 7's "a selection whose source stops
      // being live is cleared", handled here as a render-time floor even
      // before rust's own clearing lands.
      return (
        <AgentBelowBlock
          sessions={agentSessions}
          viewedIndex={viewedSessionIndex}
          capturedAtMs={agentCapturedAtMs}
          nowMs={nowMs}
        />
      );
    case "news":
      return (
        <NewsBelowBlock
          stories={NO_NEWS_STORIES}
          currentIndex={0}
          freshCount={status?.news.chargeCount ?? 0}
          cycleEndedAgo={null}
          expanded={expanded}
        />
      );
  }
}
