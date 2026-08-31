import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { useAgentState } from "@/hooks/useAgentState";
import { useAgentViewedSession } from "@/hooks/useAgentViewedSession";
import { useSlotState } from "@/hooks/useSlotState";
import { useStatusState } from "@/hooks/useStatusState";
import { useTabSelection } from "@/hooks/useTabSelection";
import { BOARD_SUMMON_MS, NOTCHTAP_EASE, SURFACE_SWAP_MS } from "@/lib/constants";
import { AgentBoard } from "./components/AgentBoard";
import { StatusRailCard } from "./components/StatusRailCard";
import { presentationMode } from "./lib/presentation";
import { presentationFacts } from "./lib/presentationFacts";

type RestingState = "rail" | "notch";

// Mirrored by HUD_CUTOUT_W/H in src-tauri/src/hover.rs; keep dimensions in lockstep.
const HUD_CUTOUT_WIDTH_PX = 200;
const HUD_CUTOUT_HEIGHT_PX = 32;

const SURFACE_STACK_STYLE = { display: "grid" } as const;
const SURFACE_CELL_STYLE = { gridArea: "1 / 1" } as const;

// Never animate transforms here: the synthetic cutout must remain fixed like hardware.
export const BOARD_SURFACE_MOTION = {
  initial: { opacity: 0 },
  animate: {
    opacity: 1,
    transition: { duration: BOARD_SUMMON_MS / 1000, ease: NOTCHTAP_EASE },
  },
  exit: {
    opacity: 0,
    transition: { duration: SURFACE_SWAP_MS / 1000, ease: NOTCHTAP_EASE },
  },
};

export const RAIL_SURFACE_MOTION = {
  initial: { opacity: 0 },
  animate: { opacity: 1 },
  exit: { opacity: 0 },
  transition: { duration: SURFACE_SWAP_MS / 1000, ease: NOTCHTAP_EASE },
};

function applyAppearance(scale: number, radius: number, opacity: number) {
  const root = document.documentElement;
  root.style.setProperty("--card-scale", String(scale));
  root.style.setProperty("--card-radius", `${radius}px`);
  root.style.setProperty("--card-opacity", String(opacity));
}

function App() {
  const slot = useSlotState();
  const status = useStatusState();
  const agentState = useAgentState();
  const mode = presentationMode(slot, agentState.sessions.length, status.paused);
  const [restingState, setRestingState] = useState<RestingState>(
    () => window.__NOTCHTAP_APPEARANCE__?.resting_state ?? "rail",
  );
  const [hovered, setHovered] = useState(false);
  // This window is receive-only: state arrives through Rust-published events; never invoke here.
  const selectedTab = useTabSelection();
  const viewedSessionIndex = useAgentViewedSession();

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
    <div className="card-root">
      <div className="surface-stack" style={SURFACE_STACK_STYLE}>
        <AnimatePresence initial={false}>
          {mode === "board" ? (
            <motion.div key="agent-board" style={SURFACE_CELL_STYLE} {...BOARD_SURFACE_MOTION}>
              <AgentBoard
                sessions={agentState.sessions}
                capturedAtMs={agentState.capturedAtMs}
                status={status}
                expanded={hovered || (slot.state === "showing" && slot.expanded)}
              />
            </motion.div>
          ) : (
            <motion.div key="status-rail" style={SURFACE_CELL_STYLE} {...RAIL_SURFACE_MOTION}>
              <StatusRailCard
                slot={slot}
                status={status}
                restingState={restingState}
                hovered={hovered}
                selectedTab={selectedTab}
                agentSessions={agentState.tabSessions}
                agentCapturedAtMs={agentState.capturedAtMs}
                viewedSessionIndex={viewedSessionIndex}
              />
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

export default App;
