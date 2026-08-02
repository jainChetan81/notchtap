import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NewsBatchHeader } from "./NewsBatchHeader";

afterEach(cleanup);

describe("NewsBatchHeader (plan 171, slice I)", () => {
  it("renders the fresh count and the cycle-ended-ago text", () => {
    const { container } = render(
      <NewsBatchHeader
        freshCount={10}
        cycleEndedAgo="2m"
        onPrevious={undefined}
        onNext={undefined}
      />,
    );
    const head = container.querySelector(".batch-head");
    expect(head).not.toBeNull();
    expect(head?.textContent).toContain("10 fresh");
    expect(head?.textContent).toContain("cycle ended 2m ago");
  });

  it("renders the separator between the count and the ago text", () => {
    const { container } = render(<NewsBatchHeader freshCount={3} cycleEndedAgo="5m" />);
    expect(container.querySelector(".batch-head .sep")?.textContent).toBe("·");
  });

  it("omits the separator and ago segment entirely when cycleEndedAgo is null", () => {
    const { container } = render(<NewsBatchHeader freshCount={0} cycleEndedAgo={null} />);
    const head = container.querySelector(".batch-head");
    expect(head?.textContent).not.toContain("cycle ended");
    expect(head?.textContent).toContain("0 fresh");
    expect(container.querySelector(".batch-head .sep")).toBeNull();
    // exactly two children survive with cycleEndedAgo null: the count
    // span and the nav wrapper — no sep/ago fragment in between.
    expect(head?.children).toHaveLength(2);
  });

  it("renders prev/next nav buttons with the mock's exact aria-labels", () => {
    const { container } = render(<NewsBatchHeader freshCount={1} cycleEndedAgo={null} />);
    const nav = container.querySelector(".batch-nav");
    expect(nav).not.toBeNull();
    const buttons = Array.from(nav?.querySelectorAll("button") ?? []);
    expect(buttons).toHaveLength(2);
    expect(buttons[0].getAttribute("aria-label")).toBe("previous story");
    expect(buttons[0].getAttribute("type")).toBe("button");
    expect(buttons[1].getAttribute("aria-label")).toBe("next story");
    expect(buttons[1].getAttribute("type")).toBe("button");
  });

  it("fires onPrevious when the previous-story button is clicked", () => {
    const onPrevious = vi.fn();
    const { container } = render(
      <NewsBatchHeader freshCount={4} cycleEndedAgo={null} onPrevious={onPrevious} />,
    );
    const button = container.querySelector('[aria-label="previous story"]') as HTMLButtonElement;
    fireEvent.click(button);
    expect(onPrevious).toHaveBeenCalledTimes(1);
  });

  it("fires onNext when the next-story button is clicked", () => {
    const onNext = vi.fn();
    const { container } = render(
      <NewsBatchHeader freshCount={4} cycleEndedAgo={null} onNext={onNext} />,
    );
    const button = container.querySelector('[aria-label="next story"]') as HTMLButtonElement;
    fireEvent.click(button);
    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it("does not crash when clicked with no callbacks wired (wiring deferred, spec section 10)", () => {
    const { container } = render(<NewsBatchHeader freshCount={2} cycleEndedAgo="1m" />);
    const buttons = container.querySelectorAll(".batch-nav button");
    expect(() => {
      for (const button of buttons) {
        fireEvent.click(button);
      }
    }).not.toThrow();
  });
});
