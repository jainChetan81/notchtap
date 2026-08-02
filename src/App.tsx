import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { BOARD_SUMMON_MS, NOTCHTAP_EASE, SURFACE_SWAP_MS } from "./animationTiming";
import { AgentBoard } from "./components/AgentBoard";
import { StatusRailCard } from "./components/StatusRailCard";
import { presentationMode } from "./lib/presentation";
import { presentationFacts } from "./lib/presentationFacts";
import { useAgentState } from "./useAgentState";
import { useSlotState } from "./useSlotState";
import { useStatusState } from "./useStatusState";

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

// 2026-08-02 animation audit (Agent Board finding #1a — the crossfade
// double-exposes with a vertical layout shift):
//
// The two top-level surfaces (Agent Board, status rail) crossfade through
// one `AnimatePresence` in its default "sync" mode, so for the length of
// the swap BOTH are mounted. They used to be plain block boxes in normal
// flow, which meant the incoming surface was pushed DOWN by the outgoing
// one's full height for the whole overlap and then snapped back up the
// frame the exit unmounted — a jump the window's `overflow: hidden` half
// hid, making it read as a flicker rather than as a swap.
//
// The fix stacks the two surfaces in ONE grid cell instead of letting
// them queue in flow. Rejected alternatives:
//   - `mode="wait"`: correct, and the simplest, but it makes the Board's
//     arrival wait out the rail's full exit first — ~180ms of dead air on
//     the one moment in this product that must not feel delayed (the
//     Board now only appears when an agent is actually blocked on the
//     operator). The swap should be continuous, not sequential.
//   - `mode="popLayout"`: absolutely positions the exiting child, which
//     needs a measurable POSITIONED parent — `.card-root` is
//     `display: contents` (styles.css) precisely so it has no box, so the
//     exiting surface would resolve against the document instead and be
//     placed wrong.
//
// So the stack is a new wrapper INSIDE `.card-root`, not a change to
// `.card-root` itself: styles.css's own comment documents that wrapper as
// a layout-neutral scoping node that "changes zero overlay geometry," and
// that guarantee is preserved here rather than amended. The new wrapper
// takes the exact box the per-surface `motion.div` already occupied (a
// full-width block child of `#root`), and each surface, as a grid item in
// the single `1 / 1` cell, stretches to that same width — so steady-state
// geometry is byte-identical to a plain block wrapper, and the only
// behavioural change is that the row's height during an overlap is the
// MAX of the two surfaces rather than their SUM. The card hangs from the
// notch (top-anchored, `transform-origin: top center`), so that max grows
// downward and neither surface ever moves.
//
// Declared as inline styles rather than a stylesheet rule because the
// mechanism belongs to this component's swap, not to the shared overlay
// card shape — and because `styles.css`'s `.card-root` block must stay as
// documented.
const SURFACE_STACK_STYLE = { display: "grid" } as const;
const SURFACE_CELL_STYLE = { gridArea: "1 / 1" } as const;

// 2026-08-02 animation audit (Agent Board finding #1b — the summon had
// zero entrance emphasis):
//
// The status rail keeps the plain symmetric crossfade it always had; the
// BOARD still gets its own, LONGER arrival clock (BOARD_SUMMON_MS, 260ms
// against the rail's 180ms exit) — but the emphasis is carried by
// duration alone, not by a transform.
//
// FEEL-CHECK RESULT (2026-08-02, operator, live): a transform entrance
// (`scale: 0.97 -> 1`, `y: -6 -> 0`, anchored `transform-origin: top
// center`) was tried and REJECTED — "looks weird, something about its
// size animation". The diagnosis: this `motion.div` wraps the WHOLE
// `.card-assembly`, synthetic notch cutout included, and that cutout's
// entire illusion is being a fixed physical anchor — card-chrome.css's
// `transform-origin` doc states the contract outright ("the notch cutout
// never moves; only the shell grows outward from it"). Scaling or
// dropping the shell moves the fake notch, which reads as the hardware
// itself twitching. Anchoring the origin at the cutout does not save it:
// the cutout still changes SIZE.
//
// LAW for any future entrance emphasis on this surface: animate ONLY
// content BELOW the cutout (the below-block, the hero, the rows), never
// the shell. `BOARD_SURFACE_MOTION` itself stays opacity-only.
//
// The exit keeps the quieter SURFACE_SWAP_MS clock, deliberately NOT the
// mirror of the entrance. That is a conscious exception to the "mirror
// the exit path exactly" spatial-consistency rule this repo otherwise
// follows (AgentBoard.tsx's `ROW_TRANSITION`): those rows are peers
// appearing and disappearing in a list, whereas this is an interruption.
// An interruption should announce itself and then leave without
// ceremony — a Board that exits as emphatically as it arrives draws the
// eye to the moment it stops mattering.
//
// Exported so App.test.tsx pins these exact values without hand-copying
// them into a second literal — the same "one const, no desynced clocks"
// discipline AgentBoard.tsx's `ROW_TRANSITION`/`HERO_SWAP_TRANSITION`
// already follow. Spread onto the `motion.div`, so pinning the object
// also pins the wiring.
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

// The status rail's own swap — untouched by the audit above: an idle rail
// coming back after a Board or a notification is the quiet, routine leg,
// and giving it the Board's arrival emphasis would flatten the very
// distinction finding #1b exists to draw.
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
  // Plan 136 (spec §6.1's presentation precedence): a Visible
  // Notification always wins; otherwise the Agent Board shows whenever
  // the independently-updating `agent-state` channel currently holds at
  // least one session AND the engine isn't Paused (operator feedback,
  // 2026-08-02: pausing must quiet the whole notch, Board included);
  // otherwise the existing clock/weather/media idle.
  // Whether a working-only set of sessions is allowed to summon the
  // Board at all (`[agents] board_show_working`, default off) is decided
  // entirely rust-side, before publish — this component just counts what
  // arrived. See lib/presentation.ts's rule-3 note.
  // `presentationMode` is pure data (lib/presentation.ts) — this is its
  // one call site.
  const agentState = useAgentState();
  const mode = presentationMode(slot, agentState.sessions.length, status.paused);
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
              />
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

export default App;
