// Plan 171 (tab-notch redesign, slice K): the selection-driven
// below-block swap. Spec `docs/superpowers/specs/2026-08-02-tab-notch-
// design.md` section 7 — "the below-block that mounts on hover-with-a-
// selection is the shipped card for that source, reused, not
// reinvented".
//
// This component owns ONLY the routing decision. Every branch below is a
// component another slice already built and tested (`AgentBelowBlock`
// slice F, `MediaBelowBlock` slice H, `NewsBelowBlock` slice I) — nothing
// here re-renders card markup of its own.
//
// **Where the two missing branches went.** Football and weather are
// deliberately NOT routed through here: spec section 7's weather bullet
// is explicit that the weather card "does not change", and section 11
// keeps `IdleHoverPeek.tsx`'s own mechanism untouched, so both selections
// reach that shipped component via its `prefer` prop instead (see
// `StatusRailCard.tsx`'s own mount site and `PeekPreference`'s doc). A
// second copy of either card here would be exactly the drift this plan's
// "reuse what's shipped" discipline exists to prevent.
//
// **Nothing selected is not a special case.** Spec section 7's "none"
// page falls out of `selected === null` returning `null` here — with no
// `.below-block` in the DOM, `card-chrome.css`'s existing
// `:not(:has(.below-block))` rounding law hands the outer corners back to
// the flanks by itself, exactly as the spec says it should.
import type { AgentSessionView } from "../useAgentState";
import type { StatusState } from "../useStatusState";
import { AgentBelowBlock } from "./AgentBelowBlock";
import type { Tab } from "./IconStrip";
import type { MediaCommand } from "./MediaBelowBlock";
import { MediaBelowBlock } from "./MediaBelowBlock";
import { NewsBelowBlock, type NewsStoryView } from "./NewsBelowBlock";

/// The tabs this component actually renders. Football/weather are
/// handled by `IdleHoverPeek`'s `prefer` prop instead (see the header
/// note) — expressed in the type so a caller can't route them here by
/// accident and silently get the "none" page.
export type TabBelowBlockTab = Exclude<Tab, "football" | "weather">;

export function tabBelowBlockHandles(selected: Tab | null): selected is TabBelowBlockTab {
  return selected === "agent" || selected === "music" || selected === "news";
}

// Transport dispatch is a rust-side concern, not a frontend one (spec
// section 10: "a click on a transport button is detected and dispatched
// to the vendored MediaRemote adapter ... from the rust side; the
// frontend never talks to the adapter directly"). Slice C already landed
// the dispatch half (`now_playing.rs`'s `MediaCommand`/`send_command`);
// what is still missing is the ROUTING half — rust needs per-button
// rects, the same way `hover.rs::icon_strip_rects` already gives it
// per-icon rects, so a real click can be attributed to prev/play-pause/
// next before it ever reaches this component. Until that lands, this is
// a genuine no-op rather than an invented `invoke()`: adding one would
// break the overlay's receive-only guarantee (CLAUDE.md's ipc/security
// section), which spec section 10 marks as a STOP-and-report condition,
// not a judgment call. Flagged follow-up, deliberately not improvised.
const NOOP_MEDIA_COMMAND = (_command: MediaCommand): void => {};

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
  /// no media session and a zero fresh-count.
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
    case "music":
      return (
        <MediaBelowBlock
          media={status?.media.current ?? null}
          expanded={expanded}
          onCommand={NOOP_MEDIA_COMMAND}
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
