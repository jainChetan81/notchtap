// Shared constants — single source for animation timings, default port, limits.
// animationTiming.ts re-exports for compat; new code imports from here.

export const DEFAULT_PORT = 9789;
export const MAX_QUEUED_PER_TIER = 50;

export {
  BOARD_SUMMON_MS,
  CONTENT_EXIT_MS,
  DISCLOSURE_SPRING,
  EXPAND_MS,
  ICON_STRIP_STAGGER_MS,
  IDLE_GLANCE_MS,
  IDLE_REVEAL_MS,
  INTERRUPT_EASE,
  INTERRUPT_EXIT_MS,
  NEWS_CHARGE_STEP_MS,
  NOTCHTAP_EASE,
  REVEAL_MS,
  ROTATION_ENTER_MS,
  ROTATION_EXIT_MS,
  SURFACE_SWAP_MS,
  SWAP_EXIT_MS,
} from "../animationTiming";
