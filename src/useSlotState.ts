import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type EventSignal =
  | "generic"
  | "goal"
  | "red_card"
  | "yellow_card"
  | "kickoff"
  | "halftime"
  | "fulltime";

export type SlotState =
  | { state: "empty" }
  | {
      state: "showing";
      id: string;
      title: string;
      body: string;
      eventType: "generic" | "score_update" | "match_state" | "news_item";
      priority: "low" | "medium" | "high";
      signal: EventSignal;
      expanded: boolean;
      source: string | null;
      category: string | null;
      publishedAtMs: number | null;
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
// itself trusted.
function isValidSlotState(v: unknown): v is SlotState {
  if (typeof v !== "object" || v === null || !("state" in v)) {
    return false;
  }
  const state = (v as { state: unknown }).state;
  return state === "empty" || state === "showing";
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
