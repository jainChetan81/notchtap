export type PresentationMode = "notch" | "hud";

declare global {
  interface Window {
    __NOTCHTAP_MODE__?: unknown;
    __NOTCHTAP_CUTOUT_WIDTH__?: unknown;
    __NOTCHTAP_CUTOUT_HEIGHT__?: unknown;
  }
}

export type PresentationFacts = {
  mode: PresentationMode;
  cutoutWidth: number | null;
  cutoutHeight: number | null;
};

import { isPositiveFiniteNumber } from "./guards";

export function presentationFacts(): PresentationFacts {
  const mode: PresentationMode = window.__NOTCHTAP_MODE__ === "notch" ? "notch" : "hud";
  const w = window.__NOTCHTAP_CUTOUT_WIDTH__;
  const cutoutWidth = isPositiveFiniteNumber(w) ? w : null;
  const h = window.__NOTCHTAP_CUTOUT_HEIGHT__;
  const cutoutHeight = isPositiveFiniteNumber(h) ? h : null;
  return { mode, cutoutWidth, cutoutHeight };
}
