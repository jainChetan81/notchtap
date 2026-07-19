import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

// plan 034: idle source-status rail. Duplicates useSlotState.ts's delivery
// discipline exactly — validator + eval-planted global seed + listener +
// dead-listener console.error — on a second, listen-only channel:
// `status-state` (rust: status.rs's STATUS_STATE_EVENT). The overlay stays
// receive-only; no invoke rides this work.
export type LiveMatchSummary = {
  label: string;
  minute: string;
};

// plan 040 Part B: the ambient weather chip's data, already display-
// formatted rust-side ("27°" + "Cloudy") — the chip concatenates them,
// same shape as football's `{live.label} · {live.minute}`.
export type WeatherSummary = {
  tempDisplay: string;
  condition: string;
};

export type StatusState = {
  paused: boolean;
  waiting: number;
  football: { enabled: boolean; live: LiveMatchSummary | null };
  news: { enabled: boolean };
  weather: { enabled: boolean; current: WeatherSummary | null };
};

declare global {
  interface Window {
    __NOTCHTAP_STATUS_STATE__?: unknown;
  }
}

// Before the first valid payload (and after any invalid one): every gate
// off, nothing queued — statusRailActive is false on it, so the idle card
// keeps its plain-clock form until rust's seed lands.
const FALLBACK_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: false },
  weather: { enabled: false, current: null },
};

// same rule as the slot-state queue-slider fields (plan 033): the rail
// renders "N queued" straight off this, so reject anything but a
// non-negative integer.
function isNonNegativeInteger(v: unknown): v is number {
  return typeof v === "number" && Number.isInteger(v) && v >= 0;
}

function isValidLiveMatch(v: unknown): v is LiveMatchSummary {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const obj = v as Record<string, unknown>;
  return typeof obj.label === "string" && typeof obj.minute === "string";
}

function isValidWeatherSummary(v: unknown): v is WeatherSummary {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const obj = v as Record<string, unknown>;
  return typeof obj.tempDisplay === "string" && typeof obj.condition === "string";
}

// Every field checked, not just the top level: a well-shaped-but-partial
// payload (e.g. football missing `enabled`) must fall back, not render
// with undefined fields — same defense-in-depth as isValidSlotState.
function isValidStatusState(v: unknown): v is StatusState {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const obj = v as Record<string, unknown>;
  if (typeof obj.football !== "object" || obj.football === null) {
    return false;
  }
  if (typeof obj.news !== "object" || obj.news === null) {
    return false;
  }
  if (typeof obj.weather !== "object" || obj.weather === null) {
    return false;
  }
  const football = obj.football as Record<string, unknown>;
  const news = obj.news as Record<string, unknown>;
  const weather = obj.weather as Record<string, unknown>;
  return (
    typeof obj.paused === "boolean" &&
    isNonNegativeInteger(obj.waiting) &&
    typeof football.enabled === "boolean" &&
    (football.live === null || isValidLiveMatch(football.live)) &&
    typeof news.enabled === "boolean" &&
    typeof weather.enabled === "boolean" &&
    (weather.current === null || isValidWeatherSummary(weather.current))
  );
}

// The rail renders only while it has something to say: a source gate on
// (football/news), a live match, items waiting behind the empty slot, or
// the engine paused. All gates off + empty queue + unpaused = the plain
// clock idle, and StatusRailCard keys the narrow 270px width off the same
// predicate (the `.rail-card.idle.status` class, plan 034).
export function statusRailActive(status: StatusState): boolean {
  return (
    status.football.enabled ||
    status.news.enabled ||
    status.football.live !== null ||
    status.weather.enabled ||
    status.weather.current !== null ||
    status.waiting > 0 ||
    status.paused
  );
}

function initialStatusState(): StatusState {
  return isValidStatusState(window.__NOTCHTAP_STATUS_STATE__)
    ? window.__NOTCHTAP_STATUS_STATE__
    : FALLBACK_STATUS;
}

export function useStatusState(): StatusState {
  const [status, setStatus] = useState<StatusState>(initialStatusState);
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<unknown>("status-state", ({ payload }) =>
      setStatus(isValidStatusState(payload) ? payload : FALLBACK_STATUS),
    )
      .then((fn) => {
        if (unmounted) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((error) => {
        // A dead listener means a permanently stale rail — make it loud
        // in the webview console since the overlay can't write to the file log.
        console.error("status-state listener failed to register", error);
      });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);
  return status;
}
