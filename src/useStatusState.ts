import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { isNonNegativeInteger } from "./lib/guards";

type UnparsedValue = string | number | boolean | null | UnparsedObject | UnparsedValue[];
type UnparsedObject = { [key: string]: UnparsedValue };

// Idle source-status rail — same delivery discipline as useSlotState.ts
// (validator + eval-planted global seed + listener + dead-listener
// console.error) on the `status-state` channel. Overlay stays receive-only.
export type LiveMatchSummary = {
  label: string;
  minute: string;
};

export type StatusState = {
  paused: boolean;
  waiting: number;
  /// Live Agent Session count — the agent icon's present/live source
  /// (present iff > 0; for agent, present IS live).
  agent: { activeSessions: number };
  football: { enabled: boolean; live: LiveMatchSummary | null };
  news: {
    enabled: boolean;
    // The news-charge cycle: chargeFraction 0..=1 fill, chargeCount items
    // waiting, isCharged held while "cycle ended with a full batch"
    // (cleared on visiting the news tab).
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
// off, nothing queued, engine unpaused.
const FALLBACK_STATUS: StatusState = {
  paused: false,
  waiting: 0,
  agent: { activeSessions: 0 },
  football: { enabled: false, live: null },
  news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
};

function isValidLiveMatch(v: unknown): v is LiveMatchSummary {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  // SAFETY: validated as record via preceding checks.
  const obj = v as UnparsedObject;
  return typeof obj.label === "string" && typeof obj.minute === "string";
}

// Every field checked, not just the top level: a well-shaped-but-partial
// payload falls back, never renders undefined fields.
function isValidStatusState(v: unknown): v is StatusState {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  // SAFETY: validated as record via preceding checks.
  const obj = v as UnparsedObject;
  if (typeof obj.agent !== "object" || obj.agent === null) {
    return false;
  }
  if (typeof obj.football !== "object" || obj.football === null) {
    return false;
  }
  if (typeof obj.news !== "object" || obj.news === null) {
    return false;
  }
  // SAFETY: validated as record via preceding checks.
  const agent = obj.agent as UnparsedObject;
  // SAFETY: validated as record via preceding checks.
  const football = obj.football as UnparsedObject;
  // SAFETY: validated as record via preceding checks.
  const news = obj.news as UnparsedObject;
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
