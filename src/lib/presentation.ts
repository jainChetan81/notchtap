// Pure presentation mapping tables for the Status Rail UI. Deliberately
// data, not logic — a config table (ARCHITECTURE.md §4's "data change,
// not a new render path" philosophy), same spirit as the frontend's
// existing event-type animation table. Every switch here is exhaustive
// (a `never` check on the default arm) so adding a new Priority/EventSignal
// variant is a compile error here until this file is updated, not a
// silent fallback to the wrong label.
import type { SlotState } from "../useSlotState";

type ShowingSlot = Extract<SlotState, { state: "showing" }>;
export type Priority = ShowingSlot["priority"];
export type EventSignal = ShowingSlot["signal"];
export type EventType = ShowingSlot["eventType"];

function assertNever(x: never): never {
  throw new Error(`unhandled case: ${JSON.stringify(x)}`);
}

export function tierCode(priority: Priority): "L1" | "M2" | "H3" {
  switch (priority) {
    case "low":
      return "L1";
    case "medium":
      return "M2";
    case "high":
      return "H3";
    default:
      return assertNever(priority);
  }
}

export function tierLabel(priority: Priority): "Low" | "Medium" | "High" {
  switch (priority) {
    case "low":
      return "Low";
    case "medium":
      return "Medium";
    case "high":
      return "High";
    default:
      return assertNever(priority);
  }
}

// Fixed per-signal text for anything with a real football signal — a
// documented lookup table, never derived from parsing title/body text
// (the cmux-notification-kind-sniffing this session already rejected).
const SIGNAL_STAMPS: Record<Exclude<EventSignal, "generic">, string> = {
  goal: "Live",
  kickoff: "Live",
  halftime: "Break",
  yellow_card: "Card",
  fulltime: "Final",
  red_card: "Off",
};

// `generic` sources (cmux/CLI/any future non-football source) have no
// specific signal to key off, so this falls back to priority alone —
// still a typed-enum lookup, not text parsing.
const GENERIC_PRIORITY_STAMPS: Record<Priority, string> = {
  low: "Live",
  medium: "Done",
  high: "Now",
};

export function stampFor(priority: Priority, signal: EventSignal): string {
  if (signal === "generic") {
    return GENERIC_PRIORITY_STAMPS[priority];
  }
  return SIGNAL_STAMPS[signal];
}

export function sourceLabelFor(eventType: EventType): string {
  switch (eventType) {
    case "generic":
      return "cmux / CLI · local";
    case "score_update":
    case "match_state":
      return "ESPN · football";
    case "news_item":
      return "RSS · news";
    default:
      return assertNever(eventType);
  }
}
