import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { isNonNegativeInteger, isNullableString } from "../lib/guards";

// The Agent Board's `agent-state` channel — same delivery discipline as
// slot-state/status-state: runtime-validated payload, dead-listener
// console.error, overlay stays receive-only. No sorting, lifecycle
// inference, expiry, or history merging here — `sessions` arrives
// already Rust-ordered and renders as-is. No `__NOTCHTAP_*__` boot-shield
// seed yet: fresh loads start empty and pick up the live channel.

const AGENT_RUNTIMES = ["claude-code", "codex", "kimi", "opencode"] as const;
export type AgentRuntime = (typeof AGENT_RUNTIMES)[number];

// Mirrors rust's `AgentSessionState` wire tokens (closed set) — an
// unrecognized state drops that session from the validated payload.
const AGENT_SESSION_STATES = [
  "starting",
  "working",
  "waiting_for_permission",
  "waiting_for_input",
  "completed",
  "failed",
  "stale",
] as const;
export type AgentSessionState = (typeof AGENT_SESSION_STATES)[number];

const AGENT_CAPABILITIES = [
  "session_lifecycle",
  "permission_requests",
  "input_required",
  "completion",
  "failure",
  "tool_details",
  "subagents",
  "open_or_focus",
] as const;
export type AgentCapability = (typeof AGENT_CAPABILITIES)[number];

export type AgentDetail = { label: string; value: string };
export type AgentProject = { name: string | null; cwd: string | null };
export type AgentHost = { name: string | null; bundleId: string | null };
// A session's active subagent, when the runtime reports one — `id` always
// present; `label`/`state` nullable like every adapter-optional string.
export type AgentSubagent = { id: string; label: string | null; state: string | null };

// One entry of a session's bounded transition history, oldest first — the
// same "no sorting" rule the wire snapshot carries. `elapsedMs` is
// clock-derived, same live-tick shape as the session's own.
export type AgentTransition = { state: AgentSessionState; elapsedMs: number };

export type AgentSessionView = {
  id: string;
  runtime: AgentRuntime;
  state: AgentSessionState;
  capabilities: AgentCapability[];
  summary: string | null;
  details: AgentDetail[];
  project: AgentProject | null;
  host: AgentHost | null;
  // `null` (not just absent) when there is no active subagent.
  subagent: AgentSubagent | null;
  // Clock-derived at capture (`capturedAtMs` is the anchor); the frontend
  // derives live time locally — a continuously-varying field never rides
  // the wire (mirrors rust's dedup exemption rule).
  elapsedMs: number;
  retentionRemainingMs: number | null;
  history: AgentTransition[];
};

export type AdapterHealthView = {
  runtime: string;
  status: string;
};

export type AgentState = {
  revision: number;
  capturedAtMs: number;
  /// The Agent BOARD's list — summons-gated on the rust side
  /// (`board.rs::gate_presence`), empty whenever nothing needs the
  /// operator; drives whether the Board is on screen.
  sessions: AgentSessionView[];
  /// The PULL surface's list — same ordered slice before the gate; the
  /// agent tab's below-block renders this one (a clicked-open tab is a
  /// user-initiated view, not a summoning). Optional on this type only
  /// because older payloads predate the field — `useAgentState` always
  /// fills it (see `sanitizeAgentState`).
  tabSessions?: AgentSessionView[];
  adapterHealth: AdapterHealthView[];
};

/// What the hook hands back: `tabSessions` always present — consumers
/// never repeat `?? []`.
export type ResolvedAgentState = AgentState & { tabSessions: AgentSessionView[] };

function emptyAgentState(): ResolvedAgentState {
  return {
    revision: 0,
    capturedAtMs: Date.now(),
    sessions: [],
    tabSessions: [],
    adapterHealth: [],
  };
}

function isDetailArray(v: unknown): v is AgentDetail[] {
  return (
    Array.isArray(v) &&
    v.every((d) => {
      if (typeof d !== "object" || d === null || !("label" in d) || !("value" in d)) {
        return false;
      }
      // SAFETY: "label" and "value" in d narrows to { label: unknown, value: unknown }.
      const pair = d as { label: unknown; value: unknown };
      return typeof pair.label === "string" && typeof pair.value === "string";
    })
  );
}

function isValidProject(v: unknown): v is AgentProject {
  if (v === null) {
    return true;
  }
  if (typeof v !== "object" || v === null || !("name" in v) || !("cwd" in v)) {
    return false;
  }
  // SAFETY: "name" and "cwd" in v narrows to { name: unknown, cwd: unknown }.
  const o = v as { name: unknown; cwd: unknown };
  return isNullableString(o.name) && isNullableString(o.cwd);
}

function isValidHost(v: unknown): v is AgentHost {
  if (v === null) {
    return true;
  }
  if (typeof v !== "object" || v === null || !("name" in v) || !("bundleId" in v)) {
    return false;
  }
  // SAFETY: "name" and "bundleId" in v narrows to { name: unknown, bundleId: unknown }.
  const o = v as { name: unknown; bundleId: unknown };
  return isNullableString(o.name) && isNullableString(o.bundleId);
}

// Same null-tolerant idiom as project/host: null, or `id` required with
// nullable `label`/`state`.
function isValidSubagent(v: unknown): v is AgentSubagent {
  if (v === null) {
    return true;
  }
  if (typeof v !== "object" || v === null || !("id" in v) || !("label" in v) || !("state" in v)) {
    return false;
  }
  // SAFETY: "id", "label", "state" in v narrows to { id: unknown, label: unknown, state: unknown }.
  const o = v as { id: unknown; label: unknown; state: unknown };
  return typeof o.id === "string" && isNullableString(o.label) && isNullableString(o.state);
}

function isValidTransition(v: unknown): v is AgentTransition {
  if (typeof v !== "object" || v === null || !("state" in v) || !("elapsedMs" in v)) {
    return false;
  }
  // SAFETY: "state" and "elapsedMs" in v narrows to { state: unknown, elapsedMs: unknown }.
  const o = v as { state: unknown; elapsedMs: unknown };
  return AGENT_SESSION_STATES.some((s) => s === o.state) && isNonNegativeInteger(o.elapsedMs);
}

function isValidSession(v: unknown): v is AgentSessionView {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  if (
    !("id" in v) ||
    !("runtime" in v) ||
    !("state" in v) ||
    !("capabilities" in v) ||
    !("summary" in v) ||
    !("details" in v) ||
    !("elapsedMs" in v)
  ) {
    return false;
  }
  // SAFETY: "id", "runtime", "state", "capabilities", "summary", "details", "elapsedMs" in v narrows to required shape.
  const o = v as {
    id: unknown;
    runtime: unknown;
    state: unknown;
    capabilities: unknown;
    summary: unknown;
    details: unknown;
    project: unknown;
    host: unknown;
    subagent: unknown;
    elapsedMs: unknown;
    retentionRemainingMs: unknown;
    history: unknown;
  };
  return (
    typeof o.id === "string" &&
    AGENT_RUNTIMES.some((r) => r === o.runtime) &&
    AGENT_SESSION_STATES.some((s) => s === o.state) &&
    Array.isArray(o.capabilities) &&
    // SAFETY: Array.isArray check above guarantees capabilities is array; widen to unknown[] for element predicate check.
    (o.capabilities as unknown[]).every((c) => AGENT_CAPABILITIES.some((a) => a === c)) &&
    isNullableString(o.summary) &&
    isDetailArray(o.details) &&
    (!("project" in v) || o.project === undefined || isValidProject(o.project)) &&
    (!("host" in v) || o.host === undefined || isValidHost(o.host)) &&
    // Absent/null-tolerant like project/host — older payloads without
    // `subagent` must not drop the session.
    (!("subagent" in v) || o.subagent === undefined || isValidSubagent(o.subagent)) &&
    isNonNegativeInteger(o.elapsedMs) &&
    (o.retentionRemainingMs === null || isNonNegativeInteger(o.retentionRemainingMs)) &&
    // Optional at validation time (defaults to `[]` below) — older
    // payloads without it must not drop the session.
    (!("history" in v) ||
      o.history === undefined ||
      (Array.isArray(o.history) && o.history.every(isValidTransition)))
  );
}

function isValidAdapterHealth(v: unknown): v is AdapterHealthView {
  if (typeof v !== "object" || v === null || !("runtime" in v) || !("status" in v)) {
    return false;
  }
  // SAFETY: "runtime" and "status" in v narrows to { runtime: unknown, status: unknown }.
  const o = v as { runtime: unknown; status: unknown };
  return typeof o.runtime === "string" && typeof o.status === "string";
}

// Arbitrary rust-serialized JSON crossing the IPC boundary — validated,
// not trusted. A malformed session is DROPPED, not the whole payload:
// one bad adapter shouldn't blank the board for every other session.
export function isValidAgentState(v: unknown): v is AgentState {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  if (
    !("revision" in v) ||
    !("capturedAtMs" in v) ||
    !("sessions" in v) ||
    !("adapterHealth" in v)
  ) {
    return false;
  }
  // SAFETY: "revision", "capturedAtMs", "sessions", "adapterHealth" in v narrows to required shape.
  const o = v as {
    revision: unknown;
    capturedAtMs: unknown;
    sessions: unknown;
    tabSessions: unknown;
    adapterHealth: unknown;
  };
  return (
    isNonNegativeInteger(o.revision) &&
    isNonNegativeInteger(o.capturedAtMs) &&
    Array.isArray(o.sessions) &&
    // Absent tolerated (older payloads); present-and-not-an-array rejects
    // the whole payload, like `sessions`. Per-session validation happens
    // in the sanitizer via `isValidSession`.
    (!("tabSessions" in v) || o.tabSessions === undefined || Array.isArray(o.tabSessions)) &&
    Array.isArray(o.adapterHealth) &&
    // SAFETY: Array.isArray check above guarantees adapterHealth is array; widen to unknown[] for element check.
    (o.adapterHealth as unknown[]).every(isValidAdapterHealth)
  );
}

// The one per-session pass both lists share — can't drift apart.
function sanitizeSessions(list: AgentSessionView[] | undefined): AgentSessionView[] {
  return (list ?? [])
    .filter(isValidSession)
    .map((s) => ({ ...s, history: s.history ?? [], subagent: s.subagent ?? null }));
}

function sanitizeAgentState(v: AgentState): ResolvedAgentState {
  return {
    ...v,
    sessions: sanitizeSessions(v.sessions),
    // Always an array after this — safe `[]` for an omitted field.
    tabSessions: sanitizeSessions(v.tabSessions),
  };
}

export const AGENT_STATE_EVENT = "agent-state";

export function useAgentState(): ResolvedAgentState {
  const [state, setState] = useState<ResolvedAgentState>(emptyAgentState);
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<unknown>(AGENT_STATE_EVENT, ({ payload }) => {
      if (isValidAgentState(payload)) {
        setState(sanitizeAgentState(payload));
      }
      // Invalid payload dropped, not blanked — keep showing the last good
      // board, never a jarring blank-then-refill.
    })
      .then((fn) => {
        if (unmounted) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((error) => {
        console.error("agent-state listener failed to register", error);
      });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);
  return state;
}
