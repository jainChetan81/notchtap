import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const EVENT_SIGNALS = [
  "generic",
  "goal",
  "red_card",
  "yellow_card",
  "kickoff",
  "halftime",
  "fulltime",
] as const;
export type EventSignal = (typeof EVENT_SIGNALS)[number];

// "news_item" added 2026-07-17 to track the backend's EventType::NewsItem
// (rss_poller.rs) — the frontend rejects unrecognized eventType values, so
// this list must stay in sync with the rust enum or real payloads silently
// fall back to empty.
const EVENT_TYPES = ["generic", "score_update", "match_state", "news_item"] as const;
type EventType = (typeof EVENT_TYPES)[number];

const PRIORITIES = ["low", "medium", "high"] as const;
type Priority = (typeof PRIORITIES)[number];

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
      expanded: boolean;
    };

declare global {
  interface Window {
    __NOTCHTAP_SLOT_STATE__?: unknown;
  }
}

// Double-shielded against the listener-registration race (2026-07-17
// review — the same race presentationMode.ts was already built for was
// never applied here): the rust core both sets
// `window.__NOTCHTAP_SLOT_STATE__` via eval AND emits a `slot-state`
// event, on every page load. If react mounts after page load, the
// initial-state read below catches the global; if it mounts before, the
// listener catches the emit. Unlike presentation-mode's fixed enum, this
// payload is arbitrary rust-serialized JSON, so it's validated rather
// than trusted blindly — defense in depth, even though the rust side is
// itself trusted. Checks every field of a "showing" payload, not just
// the state tag: a well-tagged-but-incomplete object (e.g. missing
// `signal`) must fall back to empty, not render with undefined fields.
function isValidSlotState(v: unknown): v is SlotState {
  if (typeof v !== "object" || v === null || !("state" in v)) {
    return false;
  }
  const obj = v as Record<string, unknown>;
  if (obj.state === "empty") {
    return true;
  }
  return (
    obj.state === "showing" &&
    typeof obj.id === "string" &&
    typeof obj.title === "string" &&
    typeof obj.body === "string" &&
    typeof obj.expanded === "boolean" &&
    EVENT_TYPES.includes(obj.eventType as EventType) &&
    PRIORITIES.includes(obj.priority as Priority) &&
    EVENT_SIGNALS.includes(obj.signal as EventSignal)
  );
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
    listen<SlotState>("slot-state", ({ payload }) => setSlot(payload)).then((fn) => {
      if (unmounted) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);
  return slot;
}
