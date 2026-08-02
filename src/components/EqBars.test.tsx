import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { EqBars } from "./EqBars";

afterEach(cleanup);

describe("EqBars", () => {
  it("renders three bars regardless of playing state", () => {
    const { container } = render(<EqBars playing={false} />);
    expect(container.querySelectorAll(".eq i")).toHaveLength(3);
  });

  it("carries no 'playing' class, and is aria-hidden, while silent", () => {
    const { container } = render(<EqBars playing={false} />);
    const eq = container.querySelector(".eq");
    expect(eq?.classList.contains("playing")).toBe(false);
    expect(eq?.getAttribute("aria-hidden")).toBe("true");
  });

  it("carries the 'playing' class while audio is genuinely playing", () => {
    const { container } = render(<EqBars playing={true} />);
    expect(container.querySelector(".eq")?.classList.contains("playing")).toBe(true);
  });
});
