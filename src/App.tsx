import { useEffect } from "react";
import { MotionConfig } from "motion/react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useSlotState } from "./useSlotState";
import { StatusRailCard } from "./components/StatusRailCard";
import "./styles.css";

function applyAppearance(scale: number, radius: number, opacity: number) {
  const root = document.documentElement;
  root.style.setProperty("--card-scale", String(scale));
  root.style.setProperty("--card-radius", `${radius}px`);
  root.style.setProperty("--card-opacity", String(opacity));
}

function App() {
  const slot = useSlotState();

  useEffect(() => {
    const seed = window.__NOTCHTAP_APPEARANCE__;
    if (seed) {
      applyAppearance(seed.scale, seed.radius, seed.opacity);
    }

    let unlisten: UnlistenFn | undefined;
    listen<{ scale: number; radius: number; opacity: number }>("appearance-changed", ({ payload }) => {
      applyAppearance(payload.scale, payload.radius, payload.opacity);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  return (
    <MotionConfig reducedMotion="user">
      <StatusRailCard slot={slot} />
    </MotionConfig>
  );
}

export default App;
