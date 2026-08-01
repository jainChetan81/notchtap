// Pure presentation mapping tables for the Status Rail UI. Deliberately
// data, not logic — a config table (ARCHITECTURE.md §4's "data change,
// not a new render path" philosophy), same spirit as the frontend's
// existing event-type animation table. Every switch here is exhaustive
// (a `never` check on the default arm) so adding a new Priority/EventSignal
// variant is a compile error here until this file is updated, not a
// silent fallback to the wrong label.
import type { AgentRuntime, AgentSessionState } from "../useAgentState";
import type { SlotState, SourceKind } from "../useSlotState";

type ShowingSlot = Extract<SlotState, { state: "showing" }>;
export type Priority = ShowingSlot["priority"];
export type EventSignal = ShowingSlot["signal"];
export type EventType = ShowingSlot["eventType"];

function assertNever(x: never): never {
  throw new Error(`unhandled case: ${JSON.stringify(x)}`);
}

// Fixed per-signal text for anything with a real football signal — a
// documented lookup table, never derived from parsing title/body text
// (the notification-kind-sniffing this session already rejected).
// plan 083 workstream c: minimal exhaustive-arm additions for the four
// new richer-event signals (foul/offside/var_check/substitution) — a
// wire-enum addition forces this table to compile, on purpose (the
// "seam working" the plan calls out). Visual/wording choices beyond
// this placeholder text are 084's territory.
const SIGNAL_STAMPS: Record<Exclude<EventSignal, "generic">, string> = {
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
};

// `generic` sources (agent/CLI/any future non-football source) have no
// specific signal to key off, so this falls back to priority alone —
// still a typed-enum lookup, not text parsing.
const GENERIC_PRIORITY_STAMPS: Record<Priority, string> = {
  low: "Live",
  medium: "Done",
  high: "Now",
};

export function stampFor(priority: Priority, signal: EventSignal, eventType: EventType): string {
  if (signal === "generic") {
    switch (eventType) {
      case "news_item":
        return "Wire";
      case "generic":
      case "score_update":
      case "match_state":
      // plan 135: an agent-originated card has no football signal either
      // (same as CLI/any other `generic`-signal source) — falls back to
      // priority alone, same as the other non-football event types grouped
      // here.
      case "agent_event":
        return GENERIC_PRIORITY_STAMPS[priority];
      default:
        return assertNever(eventType);
    }
  }
  return SIGNAL_STAMPS[signal];
}

// plan 084: the football scorecard's event-kind → (icon class, tint class,
// celebration) table — the ONE place a new live-match event type
// registers its presentation (see the maintenance note in plan 084's
// spec). Deliberately distinct from SIGNAL_STAMPS above, which 083 already
// populated and this plan does not touch: a wire `EventSignal` alone
// can't distinguish a regular goal from a penalty or an own goal — all
// three carry `EventSignal::Goal` (poller.rs's `labeled_detail_line`
// passes ESPN's own label through verbatim, with "Own Goal" checked
// first and short-circuited). `footballEventKindFor` below resolves the
// richer `FootballEventKind` from (signal, body) before this table is
// consulted. Reading the body's "Own Goal — "/"Penalty - Scored — "
// prefix here is NOT the rejected generic notification-kind-sniffing
// (SIGNAL_STAMPS's doc above) — it's reading a specific, tested,
// backend-guaranteed prefix that poller.rs's own tests pin byte-for-byte
// (`own_goal_body_derived_from_structural_flag`, `penalty_body_names_the_event`).
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

// Celebration values are the exact CSS class names StatusRailCard.tsx
// applies to the outer `.card-assembly` (styles.css/preview-overlay.css) —
// no separate translation table to keep in sync.
export type Celebration = "cele-goal" | "cele-yc" | "cele-rc" | null;

export interface EventKindPresentation {
  iconClass: string;
  tintClass: string | null;
  celebration: Celebration;
}

const EVENT_KIND_PRESENTATION: Record<FootballEventKind, EventKindPresentation> = {
  goal: { iconClass: "ev-ico goal", tintClass: "tint-goal", celebration: "cele-goal" },
  // penalty scored counts as a goal — same green celebration family
  // (prototype lock: "Penalty scored. Counts as a goal — same green
  // celebration."), distinct ring-shaped icon only.
  penalty_scored: { iconClass: "ev-ico pen", tintClass: "tint-goal", celebration: "cele-goal" },
  // own-goal updates the score with NO celebration — hollow-ball icon,
  // neutral (no tint) event line, per the operator-locked model.
  own_goal: { iconClass: "ev-ico og", tintClass: null, celebration: null },
  yellow_card: { iconClass: "ev-ico yc", tintClass: "tint-yc", celebration: "cele-yc" },
  red_card: { iconClass: "ev-ico rc", tintClass: "tint-rc", celebration: "cele-rc" },
  // plan 043's informational events (item 6a) open the card quietly: no
  // tint, no celebration, just the neutral CSS-shape icon.
  foul: { iconClass: "ev-ico foul", tintClass: null, celebration: null },
  offside: { iconClass: "ev-ico off", tintClass: null, celebration: null },
  var_check: { iconClass: "ev-ico var", tintClass: null, celebration: null },
  substitution: { iconClass: "ev-ico sub", tintClass: null, celebration: null },
};

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

// Resolves a wire EventSignal (+ body, for the goal family only) to the
// richer FootballEventKind the table above keys on. `null` for the three
// match-STATE signals (kickoff/halftime/fulltime) and `generic` — none of
// those is a football EVENT with an icon/tint/celebration; the live
// scorecard renders them as plain event-line text
// (StatusRailCard.tsx's live-match branch).
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

// The live scorecard's state pill (Live/Break/Final) — there is no
// pre-match "Soon" wire signal (kickoff transitions straight to Live, and
// the poller's first sighting of a match is a silent baseline — see
// StatusRailCard.tsx's live-match branch doc), so that prototype variant
// has no entry here and is deliberately omitted from the render.
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
  // plan 147: science split off from tech (see rss_poller.rs's
  // CATEGORY_KEYWORDS retarget) — its own cat-science shader class.
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

// Plan 136 (v7 ticket 4 of 13, spec §6.1): the presentation precedence
// machine — a pure data mapping, same "config table, not a new render
// path" philosophy every other function in this file already follows.
// `App.tsx` is the ONE call site: it decides between `StatusRailCard`
// (both "notification" and "idle" — the existing card already handles
// slot-empty idle rendering on its own) and the new `AgentBoard`
// component for "board", rather than teaching StatusRailCard's own
// (already elaborate) exit-choreography state machine a third mode.
//
// 1. a Visible Notification (slot.state === "showing") owns the Slot —
//    always wins, regardless of the Agent Board's own state (and
//    regardless of `paused`: rust's queue already stops PROMOTING while
//    paused, and an already-Visible Notification finishes its natural
//    Rotation, CONTEXT.md's Paused — the frontend adds no slot logic of
//    its own).
// 2. otherwise, while the engine is Paused the Board is hidden: Paused
//    means "quiet the whole notch," not just "stop promoting"
//    (operator feedback, 2026-08-02 — a paused engine still showed the
//    Board ticking with live agent activity). Note this is the opposite
//    of Silenced, which is explicitly display-layer-only and leaves the
//    idle surface — Agent Board included — behaving as normal
//    (CONTEXT.md's Silenced).
// 3. otherwise, at least one live/retained Agent Session (a non-empty
//    `sessions` array — the registry's own `terminal_retention_secs`
//    sweep is what drops a session out of that array, so "present in
//    the array at all" already means "live or retained") shows the
//    Agent Board.
// 4. otherwise, the existing clock/weather/media idle presentation.
//
// "when a noteworthy Agent Notification finishes, presentation returns
// to the still-current Agent Board" falls out for free: the instant
// `slot` goes back to "empty", this function re-evaluates against
// whatever `sessionCount` the (independently-updating) `agent-state`
// channel currently holds — there is no separate "was a board active
// before this notification" flag to thread through, because the two
// channels were never coupled to begin with.
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

// Plan 136 (spec §6.2): the resting board's "strong but non-alarming"
// visual distinction between waiting / failure / working / completed —
// a closed lookup table, same discipline as SIGNAL_STAMPS/
// EVENT_KIND_PRESENTATION above. `className` is the CSS hook
// (agent-board.css); `label` is the short state word the resting card
// renders. `starting` reads as a `working` variant (no separate visual
// language for a session's first few hundred ms) but keeps its own
// entry so a state-table lookup is total, not partial, over every wire
// value `useAgentState.ts` can deliver.
const AGENT_STATE_PRESENTATION: Record<
  AgentSessionState,
  { label: string; className: string; pulse: boolean }
> = {
  waiting_for_permission: { label: "Needs approval", className: "agent-waiting", pulse: true },
  waiting_for_input: { label: "Needs input", className: "agent-waiting", pulse: true },
  failed: { label: "Failed", className: "agent-failed", pulse: false },
  stale: { label: "Stale", className: "agent-stale", pulse: false },
  working: { label: "Working", className: "agent-working", pulse: true },
  starting: { label: "Starting", className: "agent-working", pulse: true },
  completed: { label: "Completed", className: "agent-completed", pulse: false },
};

// Mirrors rust's `agents::notification::runtime_display_name` exactly —
// the Agent Board's own display-label table (never the wire token
// itself, see AgentRuntime's own doc in useAgentState.ts).
const AGENT_RUNTIME_LABEL: Record<AgentRuntime, string> = {
  "claude-code": "Claude Code",
  codex: "Codex",
  kimi: "Kimi",
  opencode: "OpenCode",
};

export function agentRuntimeLabel(runtime: AgentRuntime): string {
  return AGENT_RUNTIME_LABEL[runtime];
}

// Plan 147: the paint-channel twin of AGENT_RUNTIME_LABEL — each agent
// runtime's identity colour class (source-identity.css), consumed
// wherever an agent-originated card needs to paint itself distinctly
// from a generic agent, football, weather, or manual/CLI card.
const AGENT_RUNTIME_CLASS: Record<AgentRuntime, string> = {
  "claude-code": "src-claude-code",
  codex: "src-codex",
  kimi: "src-kimi",
  opencode: "src-opencode",
};

export function agentRuntimeClass(runtime: AgentRuntime): string {
  return AGENT_RUNTIME_CLASS[runtime];
}

// Plan 147: resolves a card's SOURCE identity colour class
// (source-identity.css) — deliberately NOT news's business. News cards
// keep using `categoryClass` above (per-category colour, e.g.
// cat-science); callers must branch on origin === "news" themselves
// and keep calling `categoryClass` for that case rather than routing
// news through this function. `agentRuntime` is only consulted when
// `origin === "agent"`; any other origin ignores it. An agent card
// with `agentRuntime === null` (runtime not yet known/unrecognized)
// falls back to the neutral `src-agent` amber.
export function sourceClass(
  origin: Exclude<SourceKind, "news">,
  agentRuntime: AgentRuntime | null,
): string {
  switch (origin) {
    case "football":
      return "src-football";
    case "weather":
      return "src-weather";
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

// Plan 169 (step 6): the Agent Board's primary-session hero now renders
// through the SAME priority-accent channel (`--accent`/`--accent-soft`,
// `.card-assembly.low/.medium/.high`, card-chrome.css) every other
// origin's card already drives via `slot.priority` — until this plan,
// Agent Board carried no notion of "priority" at all (it painted state
// entirely through its own separate `--agent-accent` system,
// agent-board.css, untouched by this table). This is a NEW semantic,
// not a port of an existing one, so the mapping is a documented,
// implementer-chosen default rather than a spec-given constant: the two
// states that need the operator's attention NOW (a live decision or a
// hard stop) read as "high" — that's also the pairing the Target table
// (plan 169) marks "(danger tone)" on the hero's fact pills; the two
// states that are actively in flight read as "medium"; the two settled/
// quiet states read as "low". `starting` groups with `working` (same
// "in flight" family `AGENT_STATE_PRESENTATION` above already treats
// them as, both `agent-working`); `stale` groups with `completed` (both
// "nothing to do here" from the operator's perspective, just for
// different reasons).
const AGENT_STATE_PRIORITY: Record<AgentSessionState, Priority> = {
  waiting_for_permission: "high",
  waiting_for_input: "high",
  failed: "high",
  working: "medium",
  starting: "medium",
  completed: "low",
  stale: "low",
};

export function agentStatePriorityFor(state: AgentSessionState): Priority {
  return AGENT_STATE_PRIORITY[state];
}

// Plan 136 (spec §6.2's "elapsed-in-state time"): formats a duration in
// milliseconds as a short "Xs"/"Xm"/"Xh Ym" label — the Agent Board row's
// own twin of `ageLabel` above (news' "Xm ago"), but relative duration
// only, no "ago" suffix (a session's elapsed-in-state time is a live
// stopwatch, not a past timestamp). Floors, never rounds, so the label
// never reads ahead of the actual elapsed time (matching TtlBar's own
// floor-not-round countdown discipline).
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

// Plan 146 follow-up (operator feedback, 2026-07-27): the Agent Board's
// expanded rows surface `project.cwd` verbatim — an absolute path is
// wide and mostly noise once you already know it's under the user's
// home directory. This is a display-only heuristic, not a real
// `$HOME` lookup: the receive-only overlay has no OS/env access (no
// invoke commands ride the main window, CLAUDE.md's ipc/security
// section), so it pattern-matches the two conventional per-user home
// prefixes (`/Users/<name>` macOS, `/home/<name>` Linux) rather than
// resolving an actual home directory.
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
