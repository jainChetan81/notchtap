import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type SlotState =
  | { state: "empty" }
  | {
      state: "showing";
      id: string;
      title: string;
      body: string;
      eventType: "generic" | "score_update" | "match_state";
      priority: "low" | "medium" | "high";
      expanded: boolean;
    };

export function useSlotState(): SlotState {
  const [slot, setSlot] = useState<SlotState>({ state: "empty" });
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
