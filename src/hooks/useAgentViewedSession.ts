import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

export type AgentViewedSessionPayload = { index: number };

export function isValidAgentViewedSession(v: unknown): v is AgentViewedSessionPayload {
  if (typeof v !== "object" || v === null || !("index" in v)) {
    return false;
  }
  // SAFETY: "index" in v narrows v to { index: unknown } — checked above.
  const idx = (v as { index: unknown }).index;
  return typeof idx === "number" && Number.isInteger(idx) && idx >= 0;
}

export function useAgentViewedSession(): number {
  const [index, setIndex] = useState(0);
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<unknown>("agent-viewed-session-changed", ({ payload }) => {
      if (isValidAgentViewedSession(payload)) {
        setIndex(payload.index);
      }
    })
      .then((fn) => {
        if (unmounted) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((error) => {
        console.error("agent-viewed-session-changed listener failed to register", error);
      });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);
  return index;
}
