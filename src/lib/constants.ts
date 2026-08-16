// Shared constants — single source for animation timings, default port, limits.
// animationTiming.ts re-exports for compat; new code imports from here.

export const DEFAULT_PORT = 9789;
export const MAX_QUEUED_PER_TIER = 50;

export { SWAP_EXIT_MS, CONTENT_EXIT_MS, NOTCHTAP_EASE, EXPAND_MS, REVEAL_MS, ROTATION_EXIT_MS, ROTATION_ENTER_MS, INTERRUPT_EXIT_MS, INTERRUPT_EASE, SURFACE_SWAP_MS, BOARD_SUMMON_MS, DISCLOSURE_SPRING, IDLE_REVEAL_MS, IDLE_GLANCE_MS, ICON_STRIP_STAGGER_MS, NEWS_CHARGE_STEP_MS } from "../animationTiming";
