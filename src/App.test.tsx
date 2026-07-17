import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import App from "./App";
import * as useSlotStateModule from "./useSlotState";
import type { SlotState } from "./useSlotState";

vi.mock("./useSlotState");

function mockSlot(state: SlotState) {
  vi.mocked(useSlotStateModule.useSlotState).mockReturnValue(state);
}

describe("App", () => {
  it("renders nothing when the slot is empty", () => {
    mockSlot({ state: "empty" });
    const { container } = render(<App />);
    expect(container.firstChild).toBeNull();
  });

  it("renders title and body when showing", () => {
    mockSlot({
      state: "showing",
      id: "n1",
      title: "GOAL",
      body: "1-0",
      eventType: "score_update",
      priority: "high",
      expanded: false,
    });
    render(<App />);
    expect(screen.getByText("GOAL")).toBeTruthy();
    expect(screen.getByText("1-0")).toBeTruthy();
  });

  it("applies the priority class", () => {
    mockSlot({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "low",
      expanded: false,
    });
    const { container } = render(<App />);
    expect(container.querySelector(".slot.low")).not.toBeNull();
  });

  it("applies the expanded class only when expanded is true", () => {
    mockSlot({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      expanded: true,
    });
    const { container } = render(<App />);
    expect(container.querySelector(".slot.expanded")).not.toBeNull();
  });

  it("does not apply the expanded class when expanded is false", () => {
    mockSlot({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      expanded: false,
    });
    const { container } = render(<App />);
    expect(container.querySelector(".slot.expanded")).toBeNull();
  });
});
