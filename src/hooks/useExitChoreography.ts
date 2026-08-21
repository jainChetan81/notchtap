import { useMemo } from "react";
import { SWAP_EXIT_MS } from "../lib/constants";
import { presentationFacts } from "../lib/presentationFacts";
import { useDelayedSwap } from "./useDelayedSwap";
import type { SlotState } from "./useSlotState";

// The showing<->idle exit-choreography state machine. `showing` is
// recomputed internally from `slot` so the two can never drift apart
// across the hook boundary.
export function useExitChoreography(
  slot: SlotState,
  restingState: "rail" | "notch",
  hovered: boolean,
) {
  const showing = slot.state === "showing";

  // Kept for geometry choreography only — the content swap itself moved
  // to `AnimatePresence` (which owns its freeze). Geometry
  // (`geometryPriority`/`expanded`/`bare`) must NOT move into motion:
  // it needs a fake-timer-steppable JS exit window to hold the outer
  // shell's classes. Hoisted above `cardClass` so `bare` can feed it.
  const swapKey = showing ? slot.id : "idle";
  const { value: renderedSlot, exiting } = useDelayedSwap(slot, swapKey, SWAP_EXIT_MS);
  const renderedShowing = renderedSlot.state === "showing";

  // Entrance reads `renderedShowing` (promotions wait the 220ms,
  // preserving width-leads-content); exit drops immediately — the
  // below-block `motion.div`'s own exit transition supplies the duration.
  // A second delayed-swap timer would double that wait (React removes
  // the child, THEN AnimatePresence's exit runs).
  const belowBlockOpen = showing && renderedShowing;

  // The shell's geometry must stay in lockstep with the delayed-swap
  // content through the exit: once live `showing` goes false, fall back
  // to `renderedSlot` (frozen for the whole exit window) so priority/
  // expanded don't snap to idle while old content fades. Entrance applies
  // live fields immediately — `showing` is true from the first render.
  const geometryPriority = showing
    ? slot.priority
    : renderedShowing
      ? renderedSlot.priority
      : "idle";
  // `expanded` is keyboard-only. Deliberate difference from App.tsx's
  // AgentBoard `expanded={hovered}` — do not "fix" this file to match.
  const expanded = showing ? slot.expanded : renderedShowing && renderedSlot.expanded;

  // True ONLY during the showing->idle exit's freeze window, never during
  // entrance — `exiting` alone keys on both legs (a promotion re-keys
  // `swapKey` too), so `!showing` pins this to the exit leg. Drives the
  // shell's `.exiting` CSS class: width shrink and corner-round start at
  // t=0 instead of waiting on the SWAP_EXIT_MS geometry-class freeze.
  const shellExiting = !showing && renderedShowing;

  // `bare`: notch mode + fully-settled idle ("zero app-drawn pixels until
  // hovered"). Gated on `renderedShowing`/`exiting`, not the live
  // `showing` flag, so a still-exiting prior card finishes its exit.
  // Hover detection is `resting_state`-agnostic (hover.rs never reads it).
  const bare = restingState === "notch" && !renderedShowing && !exiting;

  // Narrows `shellExiting` to "exiting AND about to land on `.bare`" so
  // overlay-card.css converges the shell's width/flank paint/cutout radii
  // on the bare geometry DURING the exit window, not at the class flip
  // (kills the "box, then rounded shape pops in" artifact). Rail mode
  // never sets `bare`, so `exitToBare` is always false there and the
  // plain `.exiting` rule stays byte-identical. `!hovered` routes a
  // hovered exit onto `.exiting` too — hover already forces idle-width
  // `--cw` via `.bare.hovered`/`:has(.idle-peek)`, so converging on bare
  // would wobble (shrink to cutout, then rebound out to idle width).
  // CSS transitions retarget continuously on a mid-window hover flip.
  const exitToBare = shellExiting && restingState === "notch" && !hovered;

  // Whether the rail's painted chrome shows: bare-hover expands into the
  // full idle rail, and an arriving notification keeps that rail visible
  // above the card — one continuous shape. `bare` is already false
  // throughout any showing/exiting window, so only genuinely-bare hovers
  // need the `hovered` half of the OR.
  const railRevealed = !bare || hovered;

  // True idle only — not mid-exit, not hovered. Keyed on the delayed-
  // swap-settled basis (`renderedShowing`/`exiting`), not live `showing`
  // alone, so the face doesn't flash back before the swap settles.
  const trueIdle = !showing && !renderedShowing && !exiting && !hovered;

  // `.idle-face` is CSS-hidden for the lifetime of a real notch-hardware
  // device — the gate is the boot-time device mode (`presentationFacts`),
  // NOT `restingState`/`bare`. Read once: a boot-time global that never
  // changes, so `<IdleFace>` never mounts (its timers never arm).
  const idleFaceEligible = useMemo(() => presentationFacts().mode !== "notch", []);

  return {
    renderedSlot,
    exiting,
    renderedShowing,
    belowBlockOpen,
    geometryPriority,
    expanded,
    shellExiting,
    bare,
    exitToBare,
    railRevealed,
    trueIdle,
    idleFaceEligible,
  };
}
