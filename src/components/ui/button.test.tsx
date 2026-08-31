import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Badge } from "./badge";
import { Button } from "./button";

afterEach(cleanup);

describe("Button — transition-property", () => {
  it("keeps translate/scale in the transition property list, and drops the bare transition-all wildcard", () => {
    render(<Button>Click me</Button>);
    const button = screen.getByRole("button", { name: "Click me" });

    expect(button.className).toContain(
      "transition-[color,background-color,border-color,box-shadow,translate,scale]",
    );
    expect(button.className).not.toMatch(/(?:^|\s)transition-all(?:\s|$)/);
    expect(button.className).toContain("active:not-aria-[haspopup]:translate-y-px");
  });
});

it("Badge keeps the narrow transition-colors utility, never the transition-all wildcard", () => {
  render(<Badge>New</Badge>);
  const badge = screen.getByText("New");
  expect(badge.className).toContain("transition-colors");
  expect(badge.className).not.toMatch(/(?:^|\s)transition-all(?:\s|$)/);
});
