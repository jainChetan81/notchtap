import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { StatusRailCard } from "./components/StatusRailCard";
import { presentationFacts } from "./lib/presentationFacts";
import { useSlotState } from "./useSlotState";
import { useStatusState } from "./useStatusState";
import "./styles.css";

type RestingState = "rail" | "notch";

// plan 091: the HUD synthetic cutout — a notchless mac has no hardware
// cutout to measure, so the app draws its own pure-#000 rectangle
// (`.synthetic-cutout`, styles.css) at these dimensions instead, keeping
// the assembly's geometry formulas mode-agnostic (Decision 6: "no mode
// branch" in the shape). Mirrored in `src-tauri/src/hover.rs` as
// `HUD_CUTOUT_W`/`HUD_CUTOUT_H` — same lockstep rule as every other
// geometry constant in that file.
const HUD_CUTOUT_WIDTH_PX = 200;
const HUD_CUTOUT_HEIGHT_PX = 32;

function applyAppearance(scale: number, radius: number, opacity: number) {
  const root = document.documentElement;
  root.style.setProperty("--card-scale", String(scale));
  root.style.setProperty("--card-radius", `${radius}px`);
  root.style.setProperty("--card-opacity", String(opacity));
}

function App() {
  const slot = useSlotState();
  const status = useStatusState();
  // plan 085: the RESTING (idle) render choice, seeded like scale/radius/
  // opacity and hot-updated by the same appearance-changed listener below.
  // Missing on the seed (an old boot payload) means "rail" — the default,
  // zero-behavior-change state.
  const [restingState, setRestingState] = useState<RestingState>(
    () => window.__NOTCHTAP_APPEARANCE__?.resting_state ?? "rail",
  );
  // plan 087: the hover primitive's one diagnostic consumer — no boot
  // seed (there is nothing to seed; the cursor's start position is
  // unknown at page load), so this starts false and only ever moves via
  // the listener below.
  const [hovered, setHovered] = useState(false);

  // plan 063: expose the boot-time presentation facts to CSS — the mode
  // gates notch-only CSS, the cutout width/height feed the card-assembly's
  // geometry formulas (styles.css).
  // plan 091: in HUD mode rust never reports a cutout (there is no
  // hardware one to measure), so `cutoutWidth`/`cutoutHeight` are always
  // null there — this now falls through to the HUD synthetic constants
  // instead of leaving the vars unset, so `.synthetic-cutout` and every
  // width/flank formula have a real value to size against in both modes,
  // not just notch mode. Notch mode is unaffected: a real measurement
  // always wins over the synthetic fallback.
  useEffect(() => {
    const { mode, cutoutWidth, cutoutHeight } = presentationFacts();
    document.documentElement.dataset.notchtapMode = mode;
    const root = document.documentElement.style;
    const width = cutoutWidth ?? (mode === "hud" ? HUD_CUTOUT_WIDTH_PX : null);
    const height = cutoutHeight ?? (mode === "hud" ? HUD_CUTOUT_HEIGHT_PX : null);
    if (width !== null) {
      root.setProperty("--notchtap-cutout-width", `${width}px`);
    }
    if (height !== null) {
      root.setProperty("--notchtap-cutout-height", `${height}px`);
    }
  }, []);

  useEffect(() => {
    const seed = window.__NOTCHTAP_APPEARANCE__;
    if (seed) {
      applyAppearance(seed.scale, seed.radius, seed.opacity);
    }

    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<{
      scale: number;
      radius: number;
      opacity: number;
      resting_state?: RestingState;
    }>("appearance-changed", ({ payload }) => {
      applyAppearance(payload.scale, payload.radius, payload.opacity);
      setRestingState(payload.resting_state ?? "rail");
    })
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

  // plan 087: the hover primitive's frontend half — mirrors the
  // appearance-changed listener above exactly (the `unmounted` guard,
  // the `.catch`), because that's this repo's precedent shape for a
  // rust->webview listen-only channel. No boot-time global seed here:
  // unlike appearance/resting-state, there is no "value at page load"
  // for hover, since the seed IS the tracking area's own first event.
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let unmounted = false;
    listen<{ hovered: boolean }>("hover-changed", ({ payload }) => {
      setHovered(payload.hovered);
    })
      .then((fn) => {
        if (unmounted) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((error) => {
        console.error("hover-changed listener failed to register", error);
      });
    return () => {
      unmounted = true;
      unlisten?.();
    };
  }, []);

  return (
    <StatusRailCard slot={slot} status={status} restingState={restingState} hovered={hovered} />
  );
}

export default App;
