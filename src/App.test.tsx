import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import App from "./App";
import type { SlotState } from "./useSlotState";

type Handler = (event: { payload: SlotState }) => void;
const handlers: Handler[] = [];

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_name: string, handler: Handler) => {
    handlers.push(handler);
    return Promise.resolve(() => {});
  }),
}));

// see StatusRailCard.test.tsx for why lottie-react is stubbed
vi.mock("lottie-react", () => ({ default: () => null }));

function emit(payload: SlotState) {
  act(() => {
    handlers.forEach((handler) => handler({ payload }));
  });
}

describe("App", () => {
  beforeEach(() => {
    handlers.length = 0;
  });

  // this project's vitest config doesn't set `test.globals`, so RTL's
  // auto-cleanup (hooked off a global `afterEach`) never registers.
  afterEach(cleanup);

  it("renders the idle pill without notification content when the slot is empty", () => {
    const { container } = render(<App />);
    expect(container.querySelector(".rail-card.idle")).not.toBeNull();
    expect(container.querySelector(".title")).toBeNull();
    expect(container.querySelector(".body")).toBeNull();
  });

  it("renders title, body, and the priority class when showing", async () => {
    const { container } = render(<App />);
    emit({
      state: "showing",
      id: "n1",
      title: "GOAL",
      body: "1-0",
      eventType: "score_update",
      priority: "high",
      signal: "goal",
      expanded: false,
      source: null,
      category: null,
      publishedAtMs: null,
    });
    expect(await screen.findByText("GOAL")).toBeTruthy();
    expect(screen.getByText("1-0")).toBeTruthy();
    expect(container.querySelector(".rail-card.high")).not.toBeNull();
  });

  it("applies the expanded class only when expanded is true", async () => {
    const { container } = render(<App />);
    emit({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      signal: "generic",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
    });
    await screen.findByText("t");
    expect(container.querySelector(".rail-card.expanded")).not.toBeNull();
  });

  it("does not apply the expanded class when expanded is false", async () => {
    const { container } = render(<App />);
    emit({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      signal: "generic",
      expanded: false,
      source: null,
      category: null,
      publishedAtMs: null,
    });
    await screen.findByText("t");
    expect(container.querySelector(".rail-card.expanded")).toBeNull();
  });

  it("keeps a single card element mounted through empty, showing, and empty states", async () => {
    const { container } = render(<App />);
    const card = container.querySelector(".rail-card");

    expect(card).not.toBeNull();
    expect(card?.classList.contains("idle")).toBe(true);

    emit({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      signal: "generic",
      expanded: false,
      source: null,
      category: null,
      publishedAtMs: null,
    });
    await screen.findByText("t");
    expect(container.querySelector(".rail-card")).toBe(card);
    expect(card?.classList.contains("idle")).toBe(false);

    emit({ state: "empty" });
    // the outer card's "idle" class flips synchronously with the state
    // change, but the old title/body only leave the DOM once their exit
    // animation finishes — wait for that too, not just the class.
    await vi.waitFor(() => {
      expect(card?.classList.contains("idle")).toBe(true);
      expect(container.querySelector(".title")).toBeNull();
    });
    expect(container.querySelector(".rail-card")).toBe(card);
    expect(container.querySelector(".body")).toBeNull();
  });
});
