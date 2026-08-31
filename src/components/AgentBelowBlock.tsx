import type { AgentSessionView } from "../hooks/useAgentState";
import { agentRuntimeClass } from "../lib/presentation";
import { agentHeroPropsFor } from "./AgentBoard";
import { AgentHeroCard } from "./NotificationBody";
import { PositionBar } from "./PositionBar";

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
  viewedIndex: number;
  capturedAtMs: number;
  nowMs: number;
}) {
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
