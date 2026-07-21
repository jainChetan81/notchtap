// plan 063: boot-time presentation facts spliced by the rust core at
// page load (lib.rs's on_page_load). Mode gates notch-only CSS; the
// cutout width/height feed the card-assembly's geometry formulas
// (styles.css).
// plan 091: cutoutHeight added, mirroring cutoutWidth's exact shape and
// validation — the notch's height source (`safe_area_top_inset`), null in
// HUD mode or when the shim never reported one. App.tsx supplies the HUD
// synthetic constant when this reads null, exactly like it now does for
// width.
export type PresentationMode = "notch" | "hud";

declare global {
  interface Window {
    __NOTCHTAP_MODE__?: unknown;
    __NOTCHTAP_CUTOUT_WIDTH__?: unknown;
    __NOTCHTAP_CUTOUT_HEIGHT__?: unknown;
  }
}

export function presentationFacts(): {
  mode: PresentationMode;
  cutoutWidth: number | null;
  cutoutHeight: number | null;
} {
  const mode: PresentationMode = window.__NOTCHTAP_MODE__ === "notch" ? "notch" : "hud";
  const w = window.__NOTCHTAP_CUTOUT_WIDTH__;
  const cutoutWidth = typeof w === "number" && Number.isFinite(w) && w > 0 ? w : null;
  const h = window.__NOTCHTAP_CUTOUT_HEIGHT__;
  const cutoutHeight = typeof h === "number" && Number.isFinite(h) && h > 0 ? h : null;
  return { mode, cutoutWidth, cutoutHeight };
}
