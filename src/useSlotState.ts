import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

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
      source: string | null;
      category: string | null;
      publishedAtMs: number | null;
      link: string | null;
      // queue-slider position within the current batch (plan 033) — mirrors
      // the rust SlotState::Showing fields exactly.
      queueTotal: number;
      queueDone: number;
    };

declare global {
  interface Window {
    __NOTCHTAP_SLOT_STATE__?: unknown;
    __NOTCHTAP_APPEARANCE__?: {
      scale: number;
      radius: number;
      opacity: number;
    };
  }
}

// Double-shielded against the listener-registration race (2026-07-17
// review — the mode-delivery hook removed in plan 019 (see git history)
// was already built for this same race, but the shield was never applied
// here): the rust core both sets `window.__NOTCHTAP_SLOT_STATE__` via
// eval AND emits a `slot-state` event, on every page load. If react
// mounts after page load, the initial-state read below catches the
// global; if it mounts before, the listener catches the emit. BOTH entry
// points run their payload through `isValidSlotState` — the global via
// `initialSlotState`, every live `slot-state` event via the
// `useSlotState` listener. This payload is arbitrary rust-serialized
// JSON, so it's validated rather than trusted blindly on either path —
// defense in depth, even though the rust side is itself trusted. Checks
// every field of a "showing" payload, not just the state tag: a
// well-tagged-but-incomplete object (e.g. missing `signal`) must fall
// back to empty, not render with undefined fields.
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
    EVENT_SIGNALS.includes(obj.signal as EventSignal) &&
    (obj.source === null || typeof obj.source === "string") &&
    (obj.category === null || typeof obj.category === "string") &&
    (obj.publishedAtMs === null || typeof obj.publishedAtMs === "number") &&
    (obj.link === null || typeof obj.link === "string") &&
    isNonNegativeInteger(obj.queueTotal) &&
    isNonNegativeInteger(obj.queueDone)
  );
}

// plan 033: the slider does arithmetic on these (`floor(done * 10 / total)`)
// — a fractional or negative value would render a nonsense segment, so the
// validator rejects anything but non-negative integers.
function isNonNegativeInteger(v: unknown): v is number {
  return typeof v === "number" && Number.isInteger(v) && v >= 0;
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
        // A dead listener means a permanently frozen overlay — make it loud
        // in the webview console since the overlay can't write to the file log.
        console.error("slot-state listener failed to register", error);
      });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);
  return slot;
}
