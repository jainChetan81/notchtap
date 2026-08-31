import {
  CONTENT_EXIT_MS,
  EXPAND_MS,
  HOVER_MS,
  ICON_STRIP_STAGGER_MS,
  INTERRUPT_EXIT_MS,
  NEWS_CHARGE_STEP_MS,
  REVEAL_MS,
  ROTATION_ENTER_MS,
  ROTATION_EXIT_MS,
  SWAP_EXIT_MS,
} from "./lib/constants";

// Run before render in both entry points; missing variables invalidate CSS transition shorthands.
export function applyAnimationTiming(
  root: Pick<CSSStyleDeclaration, "setProperty"> = document.documentElement.style,
) {
  root.setProperty("--swap-exit-ms", `${SWAP_EXIT_MS}ms`);
  root.setProperty("--content-exit-ms", `${CONTENT_EXIT_MS}ms`);
  root.setProperty("--expand-ms", `${EXPAND_MS}ms`);
  root.setProperty("--reveal-ms", `${REVEAL_MS}ms`);
  root.setProperty("--hover-ms", `${HOVER_MS}ms`);
  root.setProperty("--rotation-exit-ms", `${ROTATION_EXIT_MS}ms`);
  root.setProperty("--rotation-enter-ms", `${ROTATION_ENTER_MS}ms`);
  root.setProperty("--interrupt-exit-ms", `${INTERRUPT_EXIT_MS}ms`);
  root.setProperty("--icon-strip-stagger-ms", `${ICON_STRIP_STAGGER_MS}ms`);
  root.setProperty("--news-charge-step-ms", `${NEWS_CHARGE_STEP_MS}ms`);
}
