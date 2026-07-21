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

// plan 104: the ambient now-playing snapshot. Unlike WeatherSummary
// (already display-formatted), this carries a raw snapshot — the
// frontend derives LIVE progress locally from elapsedMs/capturedAtMs
// (TtlBar.tsx's own pattern), never re-reading this on a per-second
// cadence; rust only re-emits it on a genuine adapter diff event.
export type NowPlayingSummary = {
  title: string;
  artist: string | null;
  album: string | null;
  playing: boolean;
  elapsedMs: number;
  durationMs: number | null;
  capturedAtMs: number;
  appBundleId: string | null;
};

export type StatusState = {
  paused: boolean;
  waiting: number;
  football: { enabled: boolean; live: LiveMatchSummary | null };
  news: { enabled: boolean };
  weather: { enabled: boolean; current: WeatherSummary | null };
  media: { enabled: boolean; current: NowPlayingSummary | null };
};

declare global {
  interface Window {
    __NOTCHTAP_STATUS_STATE__?: unknown;
  }
}

// Before the first valid payload (and after any invalid one): every gate
// off, nothing queued, engine unpaused — until rust's seed lands.
const FALLBACK_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: false },
  weather: { enabled: false, current: null },
  media: { enabled: false, current: null },
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

// plan 104: every field checked, matching isValidWeatherSummary's
// defense-in-depth — nullable string fields must be exactly `string |
// null`, never merely "not undefined".
function isValidNowPlaying(v: unknown): v is NowPlayingSummary {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const obj = v as Record<string, unknown>;
  return (
    typeof obj.title === "string" &&
    (obj.artist === null || typeof obj.artist === "string") &&
    (obj.album === null || typeof obj.album === "string") &&
    typeof obj.playing === "boolean" &&
    isNonNegativeInteger(obj.elapsedMs) &&
    (obj.durationMs === null || isNonNegativeInteger(obj.durationMs)) &&
    typeof obj.capturedAtMs === "number" &&
    Number.isFinite(obj.capturedAtMs) &&
    (obj.appBundleId === null || typeof obj.appBundleId === "string")
  );
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
  if (typeof obj.media !== "object" || obj.media === null) {
    return false;
  }
  const football = obj.football as Record<string, unknown>;
  const news = obj.news as Record<string, unknown>;
  const weather = obj.weather as Record<string, unknown>;
  const media = obj.media as Record<string, unknown>;
  return (
    typeof obj.paused === "boolean" &&
    isNonNegativeInteger(obj.waiting) &&
    typeof football.enabled === "boolean" &&
    (football.live === null || isValidLiveMatch(football.live)) &&
    typeof news.enabled === "boolean" &&
    typeof weather.enabled === "boolean" &&
    (weather.current === null || isValidWeatherSummary(weather.current)) &&
    typeof media.enabled === "boolean" &&
    (media.current === null || isValidNowPlaying(media.current))
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
