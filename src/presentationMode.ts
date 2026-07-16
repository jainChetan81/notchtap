import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

export type PresentationMode = "notch" | "hud";

// Default is "hud" deliberately: that's today's existing visual behavior, so
// a notification rendered before the rust core's one-shot `presentation-mode`
// event arrives still looks like it does today, not broken.
export function usePresentationMode(): PresentationMode {
  const [mode, setMode] = useState<PresentationMode>("hud");

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let unmounted = false;

    listen<{ mode: PresentationMode }>("presentation-mode", ({ payload }) => {
      setMode(payload.mode);
    }).then((fn) => {
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

  return mode;
}
