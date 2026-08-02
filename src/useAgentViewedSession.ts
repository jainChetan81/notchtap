import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

/// The frontend half of the `agent-viewed-session-changed` channel —
/// mirrors `useTabSelection.ts`'s shape exactly (listen-only, strict
/// validator, dead-listener `console.error`, no boot seed since a
/// viewed-session index is only meaningful once sessions exist).
/// **Rust owns this value, not this hook** — both the manual prefix-key
/// cycling (`handle_prefix_followup` in `src-tauri/src/lib.rs`) and the
/// auto-advance timer (this plan's Part 2) write `tab_wire.viewed_session`
/// and emit this event; the frontend only ever renders what it's told.
export type AgentViewedSessionPayload = { index: number };

export function isValidAgentViewedSession(v: unknown): v is AgentViewedSessionPayload {
  if (typeof v !== "object" || v === null) {
    return false;
  }
  const obj = v as Record<string, unknown>;
  return typeof obj.index === "number" && Number.isInteger(obj.index) && obj.index >= 0;
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
