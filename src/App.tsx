import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { MotionConfig } from "motion/react";
import { useEffect } from "react";
import { StatusRailCard } from "./components/StatusRailCard";
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

  return (
    <MotionConfig reducedMotion="user">
      <StatusRailCard slot={slot} status={status} />
    </MotionConfig>
  );
}

export default App;
