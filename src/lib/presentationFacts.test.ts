import { afterEach, describe, expect, it } from "vitest";
import { presentationFacts } from "./presentationFacts";

// mirrors useStatusState.test.ts's global-seed pattern: set/delete the
// eval-planted globals between cases so nothing leaks across tests.
afterEach(() => {
  delete window.__NOTCHTAP_MODE__;
  delete window.__NOTCHTAP_CUTOUT_WIDTH__;
});

describe("presentationFacts", () => {
  it("reads a notch mode and a positive cutout width as-is", () => {
    window.__NOTCHTAP_MODE__ = "notch";
    window.__NOTCHTAP_CUTOUT_WIDTH__ = 319;
    expect(presentationFacts()).toEqual({ mode: "notch", cutoutWidth: 319 });
  });

  it("reads hud mode explicitly", () => {
    window.__NOTCHTAP_MODE__ = "hud";
    window.__NOTCHTAP_CUTOUT_WIDTH__ = null;
    expect(presentationFacts()).toEqual({ mode: "hud", cutoutWidth: null });
  });

  it("falls back to hud on a garbage or missing mode", () => {
    window.__NOTCHTAP_MODE__ = "NOTCH";
    expect(presentationFacts().mode).toBe("hud");
    delete window.__NOTCHTAP_MODE__;
    expect(presentationFacts().mode).toBe("hud");
  });

  it("rejects zero, negative, non-number, and missing cutout widths", () => {
    window.__NOTCHTAP_MODE__ = "notch";

    window.__NOTCHTAP_CUTOUT_WIDTH__ = 0;
    expect(presentationFacts().cutoutWidth).toBeNull();

    window.__NOTCHTAP_CUTOUT_WIDTH__ = -5;
    expect(presentationFacts().cutoutWidth).toBeNull();

    window.__NOTCHTAP_CUTOUT_WIDTH__ = "319";
    expect(presentationFacts().cutoutWidth).toBeNull();

    window.__NOTCHTAP_CUTOUT_WIDTH__ = Number.NaN;
    expect(presentationFacts().cutoutWidth).toBeNull();

    delete window.__NOTCHTAP_CUTOUT_WIDTH__;
    expect(presentationFacts().cutoutWidth).toBeNull();
  });
});
