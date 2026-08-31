import { useMemo } from "react";
import { SWAP_EXIT_MS } from "../lib/constants";
import { presentationFacts } from "../lib/presentationFacts";
import { useDelayedSwap } from "./useDelayedSwap";
import type { SlotState } from "./useSlotState";

export function useExitChoreography(
  slot: SlotState,
  restingState: "rail" | "notch",
  hovered: boolean,
) {
  const showing = slot.state === "showing";

  const swapKey = showing ? slot.id : "idle";
  const { value: renderedSlot, exiting } = useDelayedSwap(slot, swapKey, SWAP_EXIT_MS);
  const renderedShowing = renderedSlot.state === "showing";

  const belowBlockOpen = showing && renderedShowing;

  const geometryPriority = showing
    ? slot.priority
    : renderedShowing
      ? renderedSlot.priority
      : "idle";
  const expanded = showing ? slot.expanded : renderedShowing && renderedSlot.expanded;

  const shellExiting = !showing && renderedShowing;

  const bare = restingState === "notch" && !renderedShowing && !exiting;

  const exitToBare = shellExiting && restingState === "notch" && !hovered;

  const railRevealed = !bare || hovered;

  const trueIdle = !showing && !renderedShowing && !exiting && !hovered;

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
