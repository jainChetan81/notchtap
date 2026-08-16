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

export type StatusState = {
  paused: boolean;
  waiting: number;
  /// Plan 171 (tab-notch): live Agent Session count — the agent icon's
  /// present/live source (present iff > 0; for agent, present IS live).
  agent: { activeSessions: number };
  football: { enabled: boolean; live: LiveMatchSummary | null };
  news: {
    enabled: boolean;
    // Plan 171 (spec §8): the news-charge cycle. chargeFraction is
    // 0..=1 fill, chargeCount items waiting, isCharged the edge-held
    // "cycle ended with a full batch" flag (cleared on visiting the
    // news tab).
    chargeFraction: number;
    chargeCount: number;
    isCharged: boolean;
  };
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
  agent: { activeSessions: 0 },
  football: { enabled: false, live: null },
  news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
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

// Every field checked, not just the top level: a well-shaped-but-partial
// payload (e.g. football missing `enabled`) must fall back, not render
// with undefined fields — same defense-in-depth as isValidSlotState.
function isValidStatusState(v: unknown): v is StatusState {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const obj = v as Record<string, unknown>;
  if (typeof obj.agent !== "object" || obj.agent === null) {
    return false;
  }
  if (typeof obj.football !== "object" || obj.football === null) {
    return false;
  }
  if (typeof obj.news !== "object" || obj.news === null) {
    return false;
  }
  const agent = obj.agent as Record<string, unknown>;
  const football = obj.football as Record<string, unknown>;
  const news = obj.news as Record<string, unknown>;
  return (
    typeof obj.paused === "boolean" &&
    isNonNegativeInteger(obj.waiting) &&
    isNonNegativeInteger(agent.activeSessions) &&
    typeof football.enabled === "boolean" &&
    (football.live === null || isValidLiveMatch(football.live)) &&
    typeof news.enabled === "boolean" &&
    typeof news.chargeFraction === "number" &&
    news.chargeFraction >= 0 &&
    news.chargeFraction <= 1 &&
    isNonNegativeInteger(news.chargeCount) &&
    typeof news.isCharged === "boolean"
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
