// Plan 171 (tab-notch redesign, slice F): the agent tab's below-block —
// mounted by whichever parent owns the icon-strip's hover-with-a-
// selection shell (slice K's integration; this component only renders
// what goes INSIDE `.below-block`, the same scope every other slice in
// this plan keeps to). Spec section 7's agent bullet, verbatim:
//
//   "the hero (one, the VIEWED session) through the same unified
//   template AgentHeroCard/AgentBoard.tsx already render ... at shipped
//   card height — hero only in compact, no roster rows. Below the hero,
//   the session position bar (§8) replaces the roster stack."
//
// Deliberately NOT `AgentBoard.tsx` reused wholesale: that component
// mounts its OWN shell (`.card-assembly.agent-board-shell`), has its own
// hover-expand mechanism (`expanded` prop, the scrollable roster list),
// and picks its "primary" session by PRIORITY (highest-ranked state
// first) rather than by an operator-driven cursor. This tab is a
// different selection axis entirely — `prefix-[`/`prefix-]` (slice D's
// `PrefixAction::PreviousSession`/`NextSession`) cycles a VIEWED index
// through ALL sessions in wire order, independent of which one AgentBoard
// would rank primary. What genuinely IS shared (the hero's title/
// subtitle/body/facts/priority derivation) is reused via
// `agentHeroPropsFor`, exported from AgentBoard.tsx for exactly this
// (see that function's own doc comment) — not a second, drifting copy.

import { agentRuntimeClass } from "../lib/presentation";
import type { AgentSessionView } from "../useAgentState";
import { agentHeroPropsFor } from "./AgentBoard";
import { AgentHeroCard } from "./NotificationBody";
import { PositionBar } from "./PositionBar";

/// `prefix-[`/`prefix-]` (spec section 9): wraps at both ends rather
/// than clamping, so cycling never dead-ends at the first/last session —
/// same reasoning `Tab::ORDER`-adjacent cycling would want on the rust
/// side, kept here as the frontend's own pure mirror since the VIEWED
/// index is frontend-local state (rust only emits which ACTION fired,
/// per slice D's `PrefixAction::PreviousSession`/`NextSession` — it does
/// not itself track a viewed index; see prefix.rs's own doc on that
/// split). A non-positive `total` returns 0, matching `PositionBar`'s
/// own "nothing to show" floor.
export function cycleSessionIndex(
  current: number,
  total: number,
  direction: "previous" | "next",
): number {
  if (total <= 0) {
    return 0;
  }
  const delta = direction === "next" ? 1 : -1;
  return (current + delta + total) % total;
}

export function AgentBelowBlock({
  sessions,
  viewedIndex,
  capturedAtMs,
  nowMs,
}: {
  sessions: AgentSessionView[];
  /// Not itself clamped by the caller — this component defends against
  /// an out-of-range index (e.g. the viewed session having just dropped
  /// out of the snapshot) the same way `PositionBar`'s own `segmentFor`
  /// clamps, rather than trusting every caller to re-derive a valid
  /// index before every render.
  viewedIndex: number;
  capturedAtMs: number;
  nowMs: number;
}) {
  // Same "nothing to show, render nothing" posture AgentBoard.tsx's own
  // `sessions.length === 0` guard uses.
  if (sessions.length === 0) {
    return null;
  }
  const clampedIndex = Math.min(Math.max(viewedIndex, 0), sessions.length - 1);
  const viewed = sessions[clampedIndex];
  const heroProps = agentHeroPropsFor(viewed, capturedAtMs, nowMs);

  return (
    <div
      className={`below-block agent-origin ${agentRuntimeClass(viewed.runtime)}`}
      data-testid="agent-below-block"
    >
      <AgentHeroCard {...heroProps} />
      <PositionBar total={sessions.length} current={clampedIndex} />
    </div>
  );
}
