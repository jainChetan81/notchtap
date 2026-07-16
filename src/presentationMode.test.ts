import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { PresentationMode } from "./presentationMode";
import { usePresentationMode } from "./presentationMode";

type Handler = (event: { payload: { mode: PresentationMode } }) => void;
const handlers: Handler[] = [];

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_name: string, handler: Handler) => {
    handlers.push(handler);
    return Promise.resolve(() => {});
  }),
}));

describe("usePresentationMode", () => {
  beforeEach(() => {
    handlers.length = 0;
    delete window.__NOTCHTAP_MODE__;
  });

  it("defaults to hud when nothing has been delivered", () => {
    const { result } = renderHook(() => usePresentationMode());
    expect(result.current).toBe("hud");
  });

  it("reads the eval-planted global as initial state (late-mount side of the race shield)", () => {
    window.__NOTCHTAP_MODE__ = "notch";
    const { result } = renderHook(() => usePresentationMode());
    expect(result.current).toBe("notch");
  });

  it("updates from the presentation-mode event (early-mount side of the race shield)", async () => {
    const { result } = renderHook(() => usePresentationMode());
    await act(async () => {
      await Promise.resolve(); // let listen() resolve and register
    });
    act(() => {
      handlers.forEach((h) => h({ payload: { mode: "notch" } }));
    });
    expect(result.current).toBe("notch");
  });

  it("ignores garbage in the global rather than rendering a broken mode", () => {
    window.__NOTCHTAP_MODE__ = "banana";
    const { result } = renderHook(() => usePresentationMode());
    expect(result.current).toBe("hud");
  });
});
