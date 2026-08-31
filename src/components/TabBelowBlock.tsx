import type { AgentSessionView } from "../hooks/useAgentState";
import type { StatusState } from "../hooks/useStatusState";
import { AgentBelowBlock } from "./AgentBelowBlock";
import type { Tab } from "./IconStrip";
import { NewsBelowBlock, type NewsStoryView } from "./NewsBelowBlock";

export type TabBelowBlockTab = Extract<Tab, "agent" | "news">;

export function tabBelowBlockHandles(selected: Tab | null): selected is TabBelowBlockTab {
  return selected === "agent" || selected === "news";
}

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
  status: StatusState | undefined;
  agentSessions: AgentSessionView[];
  agentCapturedAtMs: number;
  nowMs?: number;
  viewedSessionIndex?: number;
  expanded?: boolean;
}) {
  if (!tabBelowBlockHandles(selected)) {
    return null;
  }

  switch (selected) {
    case "agent":
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
