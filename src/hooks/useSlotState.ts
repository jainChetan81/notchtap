import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { isDetailArray, isNonNegativeInteger } from "../lib/guards";
import type { AgentRuntime } from "./useAgentState";

type UnparsedValue = string | number | boolean | null | UnparsedObject | UnparsedValue[];
type UnparsedObject = { [key: string]: UnparsedValue };

const AGENT_RUNTIMES: readonly AgentRuntime[] = ["claude-code", "codex", "kimi", "opencode"];

const EVENT_SIGNALS = [
  "generic",
  "goal",
  "red_card",
  "yellow_card",
  "kickoff",
  "halftime",
  "fulltime",
  "foul",
  "offside",
  "var_check",
  "substitution",
] as const;
export type EventSignal = (typeof EVENT_SIGNALS)[number];

const EVENT_TYPES = ["generic", "score_update", "match_state", "news_item", "agent_event"] as const;
type EventType = (typeof EVENT_TYPES)[number];

const PRIORITIES = ["low", "medium", "high"] as const;
export type Priority = (typeof PRIORITIES)[number];

const SOURCE_KINDS = ["football", "news", "manual", "agent"] as const;
export type SourceKind = (typeof SOURCE_KINDS)[number];

export interface EspnMeta {
  league: string;
  homeAbbrev: string;
  awayAbbrev: string;
  homeScore: number;
  awayScore: number;
  clock: string;
  homeCards: [number, number];
  awayCards: [number, number];
  homeCrest: string | null;
  awayCrest: string | null;
}

export type SlotState =
  | { state: "empty" }
  | {
      state: "showing";
      id: string;
      title: string;
      body: string;
      eventType: EventType;
      priority: Priority;
      signal: EventSignal;
      origin: SourceKind;
      agentRuntime: AgentRuntime | null;
      expanded: boolean;
      source: string | null;
      category: string | null;
      publishedAtMs: number | null;
      link: string | null;
      subtitle: string | null;
      details: { label: string; value: string }[];
      queueTotal: number;
      queueDone: number;
      ttlMs: number;
      remainingMs: number;
      espn?: EspnMeta;
    };

declare global {
  interface Window {
    __NOTCHTAP_SLOT_STATE__?: unknown;
    __NOTCHTAP_APPEARANCE__?: {
      scale: number;
      radius: number;
      opacity: number;
      resting_state?: "rail" | "notch";
    };
  }
}

// Boot globals and live IPC payloads share this untrusted-input validator.
function isValidSlotState(v: unknown): v is SlotState {
  if (typeof v !== "object" || v === null || !("state" in v)) {
    return false;
  }
  // SAFETY: validated as record via preceding checks.
  const obj = v as UnparsedObject;
  if (obj.state === "empty") {
    return true;
  }
  return (
    obj.state === "showing" &&
    typeof obj.id === "string" &&
    typeof obj.title === "string" &&
    typeof obj.body === "string" &&
    typeof obj.expanded === "boolean" &&
    // SAFETY: the enclosing EVENT_TYPES.includes() membership test is the runtime check that obj.eventType is a known EventType — the cast only narrows the lookup operand.
    EVENT_TYPES.includes(obj.eventType as EventType) &&
    // SAFETY: the enclosing PRIORITIES.includes() membership test is the runtime check that obj.priority is a known Priority — the cast only narrows the lookup operand.
    PRIORITIES.includes(obj.priority as Priority) &&
    // SAFETY: the enclosing EVENT_SIGNALS.includes() membership test is the runtime check that obj.signal is a known EventSignal — the cast only narrows the lookup operand.
    EVENT_SIGNALS.includes(obj.signal as EventSignal) &&
    // SAFETY: the enclosing SOURCE_KINDS.includes() membership test is the runtime check that obj.origin is a known SourceKind — the cast only narrows the lookup operand.
    SOURCE_KINDS.includes(obj.origin as SourceKind) &&
    (obj.agentRuntime === null ||
      // SAFETY: AGENT_RUNTIMES.includes is the runtime check that obj.agentRuntime is a known AgentRuntime — the cast only narrows the lookup operand after the null check.
      AGENT_RUNTIMES.includes(obj.agentRuntime as AgentRuntime)) &&
    (obj.source === null || typeof obj.source === "string") &&
    (obj.category === null || typeof obj.category === "string") &&
    (obj.publishedAtMs === null || typeof obj.publishedAtMs === "number") &&
    (obj.link === null || typeof obj.link === "string") &&
    (obj.subtitle === null || typeof obj.subtitle === "string") &&
    isDetailArray(obj.details) &&
    isNonNegativeInteger(obj.queueTotal) &&
    isNonNegativeInteger(obj.queueDone) &&
    isNonNegativeInteger(obj.ttlMs) &&
    isNonNegativeInteger(obj.remainingMs) &&
    (obj.espn === undefined || isValidEspnMeta(obj.espn))
  );
}

function isValidEspnMeta(v: unknown): v is EspnMeta {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  // SAFETY: validated as record via preceding checks.
  const o = v as UnparsedObject;
  return (
    typeof o.league === "string" &&
    typeof o.homeAbbrev === "string" &&
    typeof o.awayAbbrev === "string" &&
    isNonNegativeInteger(o.homeScore) &&
    isNonNegativeInteger(o.awayScore) &&
    typeof o.clock === "string" &&
    isCardTuple(o.homeCards) &&
    isCardTuple(o.awayCards) &&
    (o.homeCrest === null || typeof o.homeCrest === "string") &&
    (o.awayCrest === null || typeof o.awayCrest === "string")
  );
}

function isCardTuple(v: unknown): v is [number, number] {
  return Array.isArray(v) && v.length === 2 && v.every(isNonNegativeInteger);
}

function initialSlotState(): SlotState {
  return isValidSlotState(window.__NOTCHTAP_SLOT_STATE__)
    ? window.__NOTCHTAP_SLOT_STATE__
    : { state: "empty" };
}

export function useSlotState(): SlotState {
  const [slot, setSlot] = useState<SlotState>(initialSlotState);
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<unknown>("slot-state", ({ payload }) =>
      setSlot(isValidSlotState(payload) ? payload : { state: "empty" }),
    )
      .then((fn) => {
        if (unmounted) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((error) => {
        console.error("slot-state listener failed to register", error);
      });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);
  return slot;
}
