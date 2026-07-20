import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { StatusRailCard } from "./components/StatusRailCard";
import { presentationFacts } from "./lib/presentationFacts";
import { useSlotState } from "./useSlotState";
import { useStatusState } from "./useStatusState";
import "./styles.css";

function applyAppearance(scale: number, radius: number, opacity: number) {
  const root = document.documentElement;
  root.style.setProperty("--card-scale", String(scale));
  root.style.setProperty("--card-radius", `${radius}px`);
  root.style.setProperty("--card-opacity", String(opacity));
}

function App() {
  const slot = useSlotState();
  const status = useStatusState();

  // plan 063: expose the boot-time presentation facts to CSS — the mode
  // gates the notch-only width clamp, the cutout width feeds it.
  useEffect(() => {
    const { mode, cutoutWidth } = presentationFacts();
    document.documentElement.dataset.notchtapMode = mode;
    if (cutoutWidth !== null) {
      document.documentElement.style.setProperty("--notchtap-cutout-width", `${cutoutWidth}px`);
    }
  }, []);

  useEffect(() => {
    const seed = window.__NOTCHTAP_APPEARANCE__;
    if (seed) {
      applyAppearance(seed.scale, seed.radius, seed.opacity);
    }

    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<{ scale: number; radius: number; opacity: number }>(
      "appearance-changed",
      ({ payload }) => {
        applyAppearance(payload.scale, payload.radius, payload.opacity);
      },
    )
      .then((fn) => {
        if (unmounted) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((error) => {
        console.error("appearance-changed listener failed to register", error);
      });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);

  return <StatusRailCard slot={slot} status={status} />;
}

export default App;
