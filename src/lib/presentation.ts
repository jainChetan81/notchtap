import type { AgentRuntime, AgentSessionState } from "../hooks/useAgentState";
import type { SlotState, SourceKind } from "../hooks/useSlotState";

type ShowingSlot = Extract<SlotState, { state: "showing" }>;
export type Priority = ShowingSlot["priority"];
export type EventSignal = ShowingSlot["signal"];
export type EventType = ShowingSlot["eventType"];

function assertNever(x: never): never {
  throw new Error(`unhandled case: ${JSON.stringify(x)}`);
}

const SIGNAL_STAMPS = {
  goal: "Live",
  kickoff: "Live",
  halftime: "Break",
  yellow_card: "Card",
  fulltime: "Final",
  red_card: "Off",
  foul: "Foul",
  offside: "Offside",
  var_check: "VAR",
  substitution: "Sub",
} satisfies Record<Exclude<EventSignal, "generic">, string>;

const GENERIC_PRIORITY_STAMPS = {
  low: "Live",
  medium: "Done",
  high: "Now",
} satisfies Record<Priority, string>;

export function stampFor(priority: Priority, signal: EventSignal, eventType: EventType): string {
  if (signal === "generic") {
    switch (eventType) {
      case "news_item":
        return "Wire";
      case "generic":
      case "score_update":
      case "match_state":
      case "agent_event":
        return GENERIC_PRIORITY_STAMPS[priority];
      default:
        return assertNever(eventType);
    }
  }
  return SIGNAL_STAMPS[signal];
}

export type FootballEventKind =
  | "goal"
  | "penalty_scored"
  | "own_goal"
  | "yellow_card"
  | "red_card"
  | "foul"
  | "offside"
  | "var_check"
  | "substitution";

export type Celebration = "cele-goal" | "cele-yc" | "cele-rc" | null;

export interface EventKindPresentation {
  iconClass: string;
  tintClass: string | null;
  celebration: Celebration;
}

const EVENT_KIND_PRESENTATION = {
  goal: { iconClass: "ev-ico goal", tintClass: "tint-goal", celebration: "cele-goal" },
  penalty_scored: { iconClass: "ev-ico pen", tintClass: "tint-goal", celebration: "cele-goal" },
  own_goal: { iconClass: "ev-ico og", tintClass: null, celebration: null },
  yellow_card: { iconClass: "ev-ico yc", tintClass: "tint-yc", celebration: "cele-yc" },
  red_card: { iconClass: "ev-ico rc", tintClass: "tint-rc", celebration: "cele-rc" },
  foul: { iconClass: "ev-ico foul", tintClass: null, celebration: null },
  offside: { iconClass: "ev-ico off", tintClass: null, celebration: null },
  var_check: { iconClass: "ev-ico var", tintClass: null, celebration: null },
  substitution: { iconClass: "ev-ico sub", tintClass: null, celebration: null },
} satisfies Record<FootballEventKind, EventKindPresentation>;

export function eventKindPresentationFor(kind: FootballEventKind): EventKindPresentation {
  switch (kind) {
    case "goal":
    case "penalty_scored":
    case "own_goal":
    case "yellow_card":
    case "red_card":
    case "foul":
    case "offside":
    case "var_check":
    case "substitution":
      return EVENT_KIND_PRESENTATION[kind];
    default:
      return assertNever(kind);
  }
}

export function footballEventKindFor(signal: EventSignal, body: string): FootballEventKind | null {
  switch (signal) {
    case "goal":
      if (body.startsWith("Own Goal")) {
        return "own_goal";
      }
      if (body.startsWith("Penalty - Scored")) {
        return "penalty_scored";
      }
      return "goal";
    case "yellow_card":
      return "yellow_card";
    case "red_card":
      return "red_card";
    case "foul":
      return "foul";
    case "offside":
      return "offside";
    case "var_check":
      return "var_check";
    case "substitution":
      return "substitution";
    case "kickoff":
    case "halftime":
    case "fulltime":
    case "generic":
      return null;
    default:
      return assertNever(signal);
  }
}

export type LivePillVariant = "live" | "break" | "final";

export function livePillVariantFor(signal: EventSignal): LivePillVariant {
  switch (signal) {
    case "halftime":
      return "break";
    case "fulltime":
      return "final";
    case "generic":
    case "goal":
    case "red_card":
    case "yellow_card":
    case "kickoff":
    case "foul":
    case "offside":
    case "var_check":
    case "substitution":
      return "live";
    default:
      return assertNever(signal);
  }
}

const CATEGORY_CLASSES = {
  politics: "cat-politics",
  tech: "cat-tech",
  sports: "cat-sports",
  business: "cat-business",
  world: "cat-world",
  science: "cat-science",
  generic: "cat-generic",
} as const;

type KnownCategory = Exclude<keyof typeof CATEGORY_CLASSES, "generic">;

function knownCategory(category: string | null): KnownCategory | "generic" {
  switch (category) {
    case "politics":
    case "tech":
    case "sports":
    case "business":
    case "world":
    case "science":
      return category;
    default:
      return "generic";
  }
}

export function categoryClass(category: string | null): string {
  return CATEGORY_CLASSES[knownCategory(category)];
}

export function categoryLabel(category: string | null): string | null {
  if (category === null) {
    return null;
  }
  return `${category.charAt(0).toUpperCase()}${category.slice(1)}`;
}

export type PresentationMode = "notification" | "board" | "idle";

export function presentationMode(
  slot: SlotState,
  sessionCount: number,
  paused: boolean,
): PresentationMode {
  if (slot.state === "showing") {
    return "notification";
  }
  if (paused) {
    return "idle";
  }
  return sessionCount > 0 ? "board" : "idle";
}

const AGENT_STATE_PRESENTATION = {
  waiting_for_permission: { label: "Needs approval", className: "agent-waiting", pulse: true },
  waiting_for_input: { label: "Needs input", className: "agent-waiting", pulse: true },
  failed: { label: "Failed", className: "agent-failed", pulse: false },
  stale: { label: "Stale", className: "agent-stale", pulse: false },
  working: { label: "Working", className: "agent-working", pulse: true },
  starting: { label: "Starting", className: "agent-working", pulse: true },
  completed: { label: "Completed", className: "agent-completed", pulse: false },
} satisfies Record<AgentSessionState, { label: string; className: string; pulse: boolean }>;

const AGENT_RUNTIME_LABEL = {
  "claude-code": "Claude Code",
  codex: "Codex",
  kimi: "Kimi",
  opencode: "OpenCode",
} satisfies Record<AgentRuntime, string>;

export function agentRuntimeLabel(runtime: AgentRuntime): string {
  return AGENT_RUNTIME_LABEL[runtime];
}

const AGENT_RUNTIME_CLASS = {
  "claude-code": "src-claude-code",
  codex: "src-codex",
  kimi: "src-kimi",
  opencode: "src-opencode",
} satisfies Record<AgentRuntime, string>;

export function agentRuntimeClass(runtime: AgentRuntime): string {
  return AGENT_RUNTIME_CLASS[runtime];
}

export function sourceClass(
  origin: Exclude<SourceKind, "news">,
  agentRuntime: AgentRuntime | null,
): string {
  switch (origin) {
    case "football":
      return "src-football";
    case "manual":
      return "src-manual";
    case "agent":
      return agentRuntime === null ? "src-agent" : AGENT_RUNTIME_CLASS[agentRuntime];
    default:
      return assertNever(origin);
  }
}

export function agentStatePresentationFor(state: AgentSessionState): {
  label: string;
  className: string;
  pulse: boolean;
} {
  return AGENT_STATE_PRESENTATION[state];
}

const AGENT_STATE_PRIORITY = {
  waiting_for_permission: "high",
  waiting_for_input: "high",
  failed: "high",
  working: "medium",
  starting: "medium",
  completed: "low",
  stale: "low",
} satisfies Record<AgentSessionState, Priority>;

export function agentStatePriorityFor(state: AgentSessionState): Priority {
  return AGENT_STATE_PRIORITY[state];
}

export function elapsedLabel(elapsedMs: number): string {
  const totalSeconds = Math.floor(Math.max(0, elapsedMs) / 1000);
  if (totalSeconds < 60) {
    return `${totalSeconds}s`;
  }
  const totalMinutes = Math.floor(totalSeconds / 60);
  if (totalMinutes < 60) {
    return `${totalMinutes}m`;
  }
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`;
}

export function abbreviateHome(path: string): string {
  const match = path.match(/^\/(?:Users|home)\/[^/]+(\/.*)?$/);
  if (!match) {
    return path;
  }
  return `~${match[1] ?? ""}`;
}

export function ageLabel(publishedAtMs: number | null, nowMs: number): string | null {
  if (publishedAtMs === null) {
    return null;
  }

  const ageMs = Math.max(0, nowMs - publishedAtMs);
  const ageMinutes = Math.floor(ageMs / 60_000);
  if (ageMinutes < 1) {
    return "<1m ago";
  }
  if (ageMinutes < 60) {
    return `${ageMinutes}m ago`;
  }

  const ageHours = Math.floor(ageMinutes / 60);
  if (ageHours < 24) {
    return `${ageHours}h ago`;
  }
  return `${Math.floor(ageHours / 24)}d ago`;
}
