import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { BOARD_SUMMON_MS, NOTCHTAP_EASE, SURFACE_SWAP_MS } from "./animationTiming";
import { AgentBoard } from "./components/AgentBoard";
import { StatusRailCard } from "./components/StatusRailCard";
import { presentationMode } from "./lib/presentation";
import { presentationFacts } from "./lib/presentationFacts";
import { useAgentState } from "./useAgentState";
import { useAgentViewedSession } from "./useAgentViewedSession";
import { useSlotState } from "./useSlotState";
import { useStatusState } from "./useStatusState";
import { useTabSelection } from "./useTabSelection";

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
  // plain prop so StatusRailCard listens for nothing itself; no `invoke()`
  // anywhere on the path.
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

  // plan 111: `.card-root` scopes the shared card-shape stylesheet
  // (overlay-card.css) — StatusRailCard's own root element IS
  // `.card-assembly`, so this wrapper is the only ancestor available to
  // host that scope. `display: contents` (styles.css, overlay-only
  // residue) makes it a layout-neutral scoping node: it changes zero
  // overlay geometry, only which selectors match.
  // Plan 136: the Agent Board is its own top-level swap, not a mode
  // grafted into StatusRailCard's own showing<->idle exit choreography
  // (see AgentBoard.tsx's own doc for why). `initial={false}` skips the
  // entrance fade on first mount — every existing "renders synchronously"
  // assertion (App.test.tsx) stays true; only a genuine board<->rail
  // SWAP crossfades. `mode === "board"`'s own key never changes across a
  // notification<->idle transition (both map to "status-rail"), so
  // StatusRailCard itself is never remounted by this wrapper — its own
  // `.card-assembly` identity stays stable exactly as before this plan.
  // 2026-08-02 animation audit: both branches crossfade, but on
  // different clocks — the rail keeps the plain symmetric fade, the Board
  // arrives on the longer summon clock and leaves on the short shared one
  // (`BOARD_SURFACE_MOTION`/`RAIL_SURFACE_MOTION` above). Neither branch
  // transforms the shell (see `BOARD_SURFACE_MOTION`'s FEEL-CHECK RESULT),
  // and both are stacked in one grid cell so the overlap shifts nothing.
  // `initial={false}` is unchanged, so the first-mount contract above
  // holds exactly as written.
  return (
    <div className="card-root">
      {/* 2026-08-02 animation audit (finding #1a): the single-cell grid
          that keeps the two surfaces stacked instead of queued in flow
          during a swap — see `SURFACE_STACK_STYLE`'s own doc above for
          why it lives here rather than on `.card-root`, and why it
          preserves that element's documented zero-geometry guarantee. */}
      <div className="surface-stack" style={SURFACE_STACK_STYLE}>
        <AnimatePresence initial={false}>
          {mode === "board" ? (
            <motion.div key="agent-board" style={SURFACE_CELL_STYLE} {...BOARD_SURFACE_MOTION}>
              <AgentBoard
                sessions={agentState.sessions}
                capturedAtMs={agentState.capturedAtMs}
                status={status}
                // Plan 142 (v7 ticket 10 of 13, spec §6.2 expanded): the
                // SAME `hover-changed`-sourced boolean StatusRailCard's
                // own hover consumers already use — meaningful here
                // because this component is only ever mounted while
                // `mode === "board"`, so `hovered` always means "over the
                // Board" in this branch, never some other card.
                expanded={hovered}
              />
            </motion.div>
          ) : (
            <motion.div key="status-rail" style={SURFACE_CELL_STYLE} {...RAIL_SURFACE_MOTION}>
              <StatusRailCard
                slot={slot}
                status={status}
                restingState={restingState}
                hovered={hovered}
                // Plan 171 (slice K): the tab surface's three inputs.
                // `agentState` is already read above for the Board's own
                // presentation branch — the agent tab's below-block reads
                // the same snapshot rather than a second subscription.
                // Plan 177: but the OTHER list on that snapshot.
                // `sessions` is summons-gated (empty unless something
                // needs the operator, which is what keeps merely-working
                // agents from summoning the Board); `tabSessions` is the
                // ungated view the agent ICON is already lit from. A pull
                // is user-initiated, so it gets the ungated one — reading
                // `sessions` here is what made a lit icon open an empty
                // block. The Board's own render above is untouched.
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
