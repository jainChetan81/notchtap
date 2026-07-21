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
  // plan 083 workstream c: the four locked richer event kinds (foul,
  // offside, VAR check, substitution) — mirrors rust's `EventSignal`.
  "foul",
  "offside",
  "var_check",
  "substitution",
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

// plan 083: structured live-match fields (mirrors rust's `EspnMeta`) —
// present only on a Football event when `espn_live_card` is on; every
// other payload omits the `espn` key entirely (not even `null` — see
// `EventMeta.espn`'s `skip_serializing_if` doc in event.rs).
export interface EspnMeta {
  league: string;
  homeAbbrev: string;
  awayAbbrev: string;
  homeScore: number;
  awayScore: number;
  clock: string;
  homeCards: [number, number];
  awayCards: [number, number];
  // raw filesystem path to a cached crest PNG, or null on a cache miss —
  // the frontend must call `convertFileSrc` itself before using this as
  // an <img> src (plan 083 workstream a, crest route (i)).
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
      expanded: boolean;
      source: string | null;
      category: string | null;
      publishedAtMs: number | null;
      link: string | null;
      // rich-relay fields (plan 035) — mirror the rust SlotState::Showing.
      // The manifest renders `subtitle` as its own cell and one cell per
      // `details` pair; `details` is always an array (never null).
      subtitle: string | null;
      details: { label: string; value: string }[];
      // queue-slider position within the current batch (plan 033) — mirrors
      // the rust SlotState::Showing fields exactly.
      queueTotal: number;
      queueDone: number;
      // TTL-bar timing (plan 081) — mirrors the rust SlotState::Showing
      // fields exactly. `remainingMs` is a snapshot at emission time; the
      // frontend anchors its own countdown from it on receipt (see
      // TtlBar.tsx) rather than re-reading it every frame.
      ttlMs: number;
      remainingMs: number;
      // plan 083: optional — absent on every non-football payload and on
      // football with `espn_live_card` off (the key itself is omitted on
      // the wire, so this reads as `undefined`, not `null`).
      espn?: EspnMeta;
    };

declare global {
  interface Window {
    __NOTCHTAP_SLOT_STATE__?: unknown;
    __NOTCHTAP_APPEARANCE__?: {
      scale: number;
      radius: number;
      opacity: number;
      // plan 085: optional — an old seed predating this field means
      // `rail` (the frontend default), never a hard failure.
      resting_state?: "rail" | "notch";
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
    (obj.subtitle === null || typeof obj.subtitle === "string") &&
    isDetailArray(obj.details) &&
    isNonNegativeInteger(obj.queueTotal) &&
    isNonNegativeInteger(obj.queueDone) &&
    isNonNegativeInteger(obj.ttlMs) &&
    isNonNegativeInteger(obj.remainingMs) &&
    // plan 083: espn is entirely optional (the wire omits the key rather
    // than sending null) — absent must still validate; present-but-
    // malformed must fall back like every other field.
    (obj.espn === undefined || isValidEspnMeta(obj.espn))
  );
}

// plan 083: `espn`'s nested-object check — absent or valid, never a
// half-populated block (a malformed espn must fall back like every other
// field, same discipline as `isDetailArray` above).
function isValidEspnMeta(v: unknown): v is EspnMeta {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const o = v as Record<string, unknown>;
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

// plan 035: `details` is an array of {label, value} string pairs. Like every
// other field it's revalidated here even though the rust side is trusted —
// the pairs originate in untrusted hook input, so a malformed `details`
// (a non-array, or an item missing string label/value) falls back to empty.
function isDetailArray(v: unknown): v is { label: string; value: string }[] {
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
