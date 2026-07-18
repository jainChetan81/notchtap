import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

export type PresentationMode = "notch" | "hud";

declare global {
  interface Window {
    __NOTCHTAP_MODE__?: string;
  }
}

// Mode delivery is double-shielded against the listener-registration race
// (2026-07-17 review): the rust core both sets `window.__NOTCHTAP_MODE__`
// via eval AND emits a `presentation-mode` event at every page load. If
// react mounts after page load, the initial-state read below catches the
// global; if it mounts before, the listener catches the emit. "hud" is the
// fallback when neither has landed yet — today's existing visual behavior,
// so an early-rendered notification looks normal, not broken.
function initialMode(): PresentationMode {
  return window.__NOTCHTAP_MODE__ === "notch" ? "notch" : "hud";
}

export function usePresentationMode(): PresentationMode {
  const [mode, setMode] = useState<PresentationMode>(initialMode);

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
