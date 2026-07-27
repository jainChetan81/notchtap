import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

// Plan 136 (v7 ticket 4 of 13, spec §6): the Agent Board's `agent-state`
// channel. Duplicates useSlotState.ts's/useStatusState.ts's delivery
// discipline — runtime-validated payload, dead-listener console.error —
// on a third, listen-only channel: `agent-state` (rust:
// agents/board.rs::AGENT_STATE_EVENT). The overlay stays receive-only;
// no invoke rides this work, and there is deliberately NO sorting,
// lifecycle inference, expiry, or history merging here — `sessions`
// arrives already Rust-ordered (spec §2.2) and this hook renders it
// as-is (spec §6's own words).
//
// Unlike slot-state/status-state, there is no `window.__NOTCHTAP_*__`
// boot-shield seed for this channel yet — a fresh page load starts at
// the empty snapshot below and picks up the live channel from the next
// `/agent/events` mutation or periodic tick's publish. A future ticket
// can add the same eval-planted-global dual-path shield those two
// channels use if the reload gap turns out to matter in practice.

const AGENT_RUNTIMES = ["claude-code", "codex", "kimi", "opencode"] as const;
export type AgentRuntime = (typeof AGENT_RUNTIMES)[number];

// Mirrors rust's `AgentSessionState` wire tokens exactly
// (`agents::adapter::state_wire_label`) — a closed set, same rejection
// discipline as every other enum on this app's wires (SOURCE_KINDS,
// EVENT_TYPES, ...): an unrecognized state drops that session from the
// validated payload rather than rendering with an undefined state.
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
// Plan 147 wave 2: a session's own active subagent, when the runtime
// reports one (`subagents` capability territory) — `id` is always
// present when the object itself is present, `label`/`state` are
// nullable exactly like every other adapter-optional string field on
// this type (AgentProject.name/cwd, AgentHost.name/bundleId).
export type AgentSubagent = { id: string; label: string | null; state: string | null };

// Plan 142 (v7 ticket 10 of 13, spec §6.2 expanded): one entry of a
// session's bounded transition history (rust: `agents::board::
// AgentTransitionView`), oldest first — the same "no sorting" rule the
// wire snapshot as a whole already carries applies here too.
// `elapsedMs` is clock-derived (milliseconds since that transition
// started, as of `capturedAtMs`) — same live-tick shape as the session's
// own `elapsedMs`.
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
  // Plan 147 wave 2: the session's active subagent, when the runtime
  // reports one — `null` (not just absent) when there is none.
  subagent: AgentSubagent | null;
  // Clock-derived at the moment rust captured this snapshot
  // (`capturedAtMs` below is the shared anchor) — the frontend derives
  // LIVE elapsed-in-state time locally: `elapsedMs + (Date.now() -
  // capturedAtMs)`, same pattern `NowPlayingSummary`'s
  // elapsedMs/capturedAtMs pair already uses (useStatusState.ts).
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
  sessions: AgentSessionView[];
  adapterHealth: AdapterHealthView[];
};

function emptyAgentState(): AgentState {
  return { revision: 0, capturedAtMs: Date.now(), sessions: [], adapterHealth: [] };
}

function isNonNegativeInteger(v: unknown): v is number {
  return typeof v === "number" && Number.isInteger(v) && v >= 0;
}

function isNullableString(v: unknown): v is string | null {
  return v === null || typeof v === "string";
}

function isDetailArray(v: unknown): v is AgentDetail[] {
  return (
    Array.isArray(v) &&
    v.every((d) => {
      if (typeof d !== "object" || d === null) {
        return false;
      }
      const pair = d as Record<string, unknown>;
      return typeof pair.label === "string" && typeof pair.value === "string";
    })
  );
}

function isValidProject(v: unknown): v is AgentProject {
  if (v === null) {
    return true;
  }
  if (typeof v !== "object") {
    return false;
  }
  const o = v as Record<string, unknown>;
  return isNullableString(o.name) && isNullableString(o.cwd);
}

function isValidHost(v: unknown): v is AgentHost {
  if (v === null) {
    return true;
  }
  if (typeof v !== "object") {
    return false;
  }
  const o = v as Record<string, unknown>;
  return isNullableString(o.name) && isNullableString(o.bundleId);
}

// Mirrors isValidProject/isValidHost's exact idiom: the value itself
// may be `null` (no active subagent), and when present, `id` is
// required while `label`/`state` are null-tolerant like every other
// adapter-optional string field on this wire.
function isValidSubagent(v: unknown): v is AgentSubagent {
  if (v === null) {
    return true;
  }
  if (typeof v !== "object") {
    return false;
  }
  const o = v as Record<string, unknown>;
  return typeof o.id === "string" && isNullableString(o.label) && isNullableString(o.state);
}

function isValidTransition(v: unknown): v is AgentTransition {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const o = v as Record<string, unknown>;
  return (
    AGENT_SESSION_STATES.includes(o.state as AgentSessionState) && isNonNegativeInteger(o.elapsedMs)
  );
}

function isValidSession(v: unknown): v is AgentSessionView {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const o = v as Record<string, unknown>;
  return (
    typeof o.id === "string" &&
    AGENT_RUNTIMES.includes(o.runtime as AgentRuntime) &&
    AGENT_SESSION_STATES.includes(o.state as AgentSessionState) &&
    Array.isArray(o.capabilities) &&
    (o.capabilities as unknown[]).every((c) => AGENT_CAPABILITIES.includes(c as AgentCapability)) &&
    isNullableString(o.summary) &&
    isDetailArray(o.details) &&
    (o.project === undefined || isValidProject(o.project)) &&
    (o.host === undefined || isValidHost(o.host)) &&
    // Plan 147 wave 2: same absent/null-tolerant idiom as project/host
    // above — an older cached payload without `subagent` at all must
    // not drop the session either.
    (o.subagent === undefined || isValidSubagent(o.subagent)) &&
    isNonNegativeInteger(o.elapsedMs) &&
    (o.retentionRemainingMs === null || isNonNegativeInteger(o.retentionRemainingMs)) &&
    // Plan 142: `history` is optional at validation time (defaults to
    // `[]` below) — an older cached/boot payload without it must not
    // drop the whole session, same "degrade, don't crash" discipline
    // every other optional field on this type already follows.
    (o.history === undefined || (Array.isArray(o.history) && o.history.every(isValidTransition)))
  );
}

function isValidAdapterHealth(v: unknown): v is AdapterHealthView {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const o = v as Record<string, unknown>;
  return typeof o.runtime === "string" && typeof o.status === "string";
}

// Validated rather than trusted blindly, same defense-in-depth rationale
// isValidSlotState's own doc gives (useSlotState.ts): this is arbitrary
// rust-serialized JSON crossing the tauri IPC boundary. A malformed
// individual session is DROPPED (not the whole payload) — one adapter
// sending a bad event shouldn't blank the entire board out from under
// every other session's row.
export function isValidAgentState(v: unknown): v is AgentState {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const o = v as Record<string, unknown>;
  return (
    isNonNegativeInteger(o.revision) &&
    isNonNegativeInteger(o.capturedAtMs) &&
    Array.isArray(o.sessions) &&
    Array.isArray(o.adapterHealth) &&
    (o.adapterHealth as unknown[]).every(isValidAdapterHealth)
  );
}

function sanitizeAgentState(v: AgentState): AgentState {
  return {
    ...v,
    sessions: v.sessions
      .filter(isValidSession)
      .map((s) => ({ ...s, history: s.history ?? [], subagent: s.subagent ?? null })),
  };
}

export const AGENT_STATE_EVENT = "agent-state";

export function useAgentState(): AgentState {
  const [state, setState] = useState<AgentState>(emptyAgentState);
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<unknown>(AGENT_STATE_EVENT, ({ payload }) => {
      if (isValidAgentState(payload)) {
        setState(sanitizeAgentState(payload));
      }
      // an invalid payload is dropped, not blanked to empty — mirrors
      // `isValidSlotState`'s own comment: a well-tagged-but-incomplete
      // object must fall back safely, but here "safely" means "keep
      // showing the last good board," never a jarring blank-then-refill.
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
