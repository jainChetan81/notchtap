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

// Fixed per-signal text for anything with a real football signal — a
// documented lookup table, never derived from parsing title/body text
// (the cmux-notification-kind-sniffing this session already rejected).
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

// `generic` sources (cmux/CLI/any future non-football source) have no
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
        return GENERIC_PRIORITY_STAMPS[priority];
      default:
        return assertNever(eventType);
    }
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
      return "RSS · news wire";
    default:
      return assertNever(eventType);
  }
}

const CATEGORY_CLASSES = {
  politics: "cat-politics",
  tech: "cat-tech",
  sports: "cat-sports",
  business: "cat-business",
  world: "cat-world",
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

export function publishedLabel(publishedAtMs: number | null, _nowMs: number): string | null {
  if (publishedAtMs === null) {
    return null;
  }
  const published = new Date(publishedAtMs);
  const hours = published.getHours().toString().padStart(2, "0");
  const minutes = published.getMinutes().toString().padStart(2, "0");
  return `${hours}:${minutes}`;
}
