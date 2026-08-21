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

// HUD mode has no hardware cutout to measure, so the app draws its own
// pure-#000 rectangle (`.synthetic-cutout`, styles.css) at these
// dimensions. Mirrored as `HUD_CUTOUT_W`/`HUD_CUTOUT_H` in
// src-tauri/src/hover.rs — same lockstep rule as its geometry constants.
const HUD_CUTOUT_WIDTH_PX = 200;
const HUD_CUTOUT_HEIGHT_PX = 32;

// Both surfaces stack in one grid cell so the crossfade overlap never
// shifts layout: the row's height is the max of the two, growing down
// from the notch. `mode="wait"` would add dead air before the Board;
// `popLayout` needs a positioned parent and `.card-root` is display:contents.
const SURFACE_STACK_STYLE = { display: "grid" } as const;
const SURFACE_CELL_STYLE = { gridArea: "1 / 1" } as const;

// The Board arrives on the longer BOARD_SUMMON_MS clock and exits on the
// quieter SURFACE_SWAP_MS — emphasis carried by duration alone. LAW:
// never transform the shell (the cutout must read as fixed hardware);
// animate only content below the cutout, opacity-only here.
// Exported so App.test.tsx pins these exact values.
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

// The rail's routine swap — a plain symmetric crossfade, deliberately
// without the Board's arrival emphasis.
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
  // Precedence: a Visible Notification always wins; else the Board shows
  // when `agent-state` holds sessions and the engine isn't Paused; else
  // clock idle. Working-only summons are gated rust-side before publish.
  // `presentationMode` is pure data (lib/presentation.ts) — one call site.
  const agentState = useAgentState();
  const mode = presentationMode(slot, agentState.sessions.length, status.paused);
  // Resting (idle) render choice — seeded like scale/radius/opacity, hot-
  // updated by the appearance-changed listener; a missing seed means "rail".
  const [restingState, setRestingState] = useState<RestingState>(
    () => window.__NOTCHTAP_APPEARANCE__?.resting_state ?? "rail",
  );
  // No boot seed — the cursor position at page load is unknown, so this
  // starts false and only moves via the hover-changed listener below.
  const [hovered, setHovered] = useState(false);
  // Which tab rust has selected — decided rust-side, threaded down as a
  // plain prop so StatusRailCard listens for nothing itself. The overlay
  // is receive-only: it listens for rust-published events, invokes nothing.
  const selectedTab = useTabSelection();
  // The Agent tab's viewed-session cursor — rust owns the value; this
  // hook only renders what it's told.
  const viewedSessionIndex = useAgentViewedSession();

  // Expose boot-time presentation facts to CSS — mode gates notch-only
  // rules, cutout size feeds the geometry formulas (styles.css). HUD mode
  // reports no cutout, so the synthetic constants fill in; a real
  // measurement always wins.
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

  // Mirrors the appearance-changed listener's shape above. No boot seed:
  // the tracking area's own first event is the seed.
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

  // `.card-root` scopes overlay-card.css onto StatusRailCard, whose own
  // root IS `.card-assembly` — this wrapper is the only ancestor hosting
  // that scope; `display: contents` keeps it layout-neutral.
  // The Agent Board is a top-level swap, not a mode inside StatusRailCard:
  // `initial={false}` keeps first-mount renders synchronous (App.test.tsx),
  // and the `agent-board` key changes only on a genuine board<->rail swap
  // (notification<->idle both map to "status-rail"), so StatusRailCard is
  // never remounted by this wrapper.
  // Both branches crossfade opacity-only on different clocks; never
  // transform the shell (the cutout must read as fixed hardware).
  return (
    <div className="card-root">
      {/* Single-cell grid keeps the two surfaces stacked instead of queued
          in flow during a swap — see `SURFACE_STACK_STYLE` above. */}
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
                // Tab surface inputs: `agentState` read above for the
                // Board's own branch — same snapshot, no second
                // subscription. `sessions` is summons-gated, `tabSessions`
                // the ungated view the agent ICON is lit from; a pull is
                // user-initiated, so it reads the ungated one — `sessions`
                // would open an empty block. Board render untouched.
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
