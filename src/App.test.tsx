import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
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

function emit(payload: SlotState) {
  act(() => {
    handlers.forEach((handler) => handler({ payload }));
  });
}

describe("App", () => {
  beforeEach(() => {
    handlers.length = 0;
  });

  it("renders an idle pill without notification content when the slot is empty", () => {
    const { container } = render(<App />);
    expect(container.querySelector(".slot.idle")).not.toBeNull();
    expect(container.querySelector(".title")).toBeNull();
    expect(container.querySelector(".body")).toBeNull();
  });

  it("renders title, body, and the priority class when showing", () => {
    const { container } = render(<App />);
    emit({
      state: "showing",
      id: "n1",
      title: "GOAL",
      body: "1-0",
      eventType: "score_update",
      priority: "high",
      expanded: false,
    });
    expect(screen.getByText("GOAL")).toBeTruthy();
    expect(screen.getByText("1-0")).toBeTruthy();
    expect(container.querySelector(".slot.high")).not.toBeNull();
  });

  it("applies the expanded class only when expanded is true", () => {
    const { container } = render(<App />);
    emit({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      expanded: true,
    });
    expect(container.querySelector(".slot.expanded")).not.toBeNull();
  });

  it("does not apply the expanded class when expanded is false", () => {
    const { container } = render(<App />);
    emit({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      expanded: false,
    });
    expect(container.querySelector(".slot.expanded")).toBeNull();
  });

  it("keeps the outer slot mounted through empty, showing, and empty states", () => {
    const { container } = render(<App />);
    const outerSlot = container.querySelector(".slot");

    expect(outerSlot).not.toBeNull();
    expect(outerSlot?.classList.contains("idle")).toBe(true);

    emit({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      expanded: false,
    });
    expect(container.querySelector(".slot")).toBe(outerSlot);
    expect(outerSlot?.classList.contains("idle")).toBe(false);

    emit({ state: "empty" });
    expect(container.querySelector(".slot")).toBe(outerSlot);
    expect(outerSlot?.classList.contains("idle")).toBe(true);
    expect(container.querySelector(".title")).toBeNull();
    expect(container.querySelector(".body")).toBeNull();
  });
});
