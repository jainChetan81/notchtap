// plan 063: boot-time presentation facts spliced by the rust core at
// page load (lib.rs's on_page_load). Mode gates notch-only CSS; the
// cutout width feeds the idle status rail's width clamp (styles.css).
export type PresentationMode = "notch" | "hud";

declare global {
  interface Window {
    __NOTCHTAP_MODE__?: unknown;
    __NOTCHTAP_CUTOUT_WIDTH__?: unknown;
  }
}

export function presentationFacts(): { mode: PresentationMode; cutoutWidth: number | null } {
  const mode: PresentationMode = window.__NOTCHTAP_MODE__ === "notch" ? "notch" : "hud";
  const w = window.__NOTCHTAP_CUTOUT_WIDTH__;
  const cutoutWidth = typeof w === "number" && Number.isFinite(w) && w > 0 ? w : null;
  return { mode, cutoutWidth };
}
