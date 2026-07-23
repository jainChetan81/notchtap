import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Badge } from "./badge";
import { Button } from "./button";

// this project's vitest config doesn't set `test.globals`, so RTL's
// auto-cleanup (which hooks a global `afterEach`) never registers.
afterEach(cleanup);

// plan 126 (finding #10): the bare `transition-all` this component used to
// carry animated every property change, including ones with no visual
// transition author ever intended (a broad, imprecise wildcard). Swapping
// it for an explicit property list must keep `transform` in that list —
// it's what makes `active:not-aria-[haspopup]:translate-y-px`'s press
// feedback actually animate instead of snapping. This is a string-level
// pin, not a computed-style assertion: jsdom doesn't run CSS transitions,
// so the only thing to assert is that the utility class carries the right
// transition-property list.
describe("Button — transition-property (plan 126)", () => {
  it("keeps transform in the transition property list, and drops the bare transition-all wildcard", () => {
    render(<Button>Click me</Button>);
    const button = screen.getByRole("button", { name: "Click me" });

    expect(button.className).toContain(
      "transition-[color,background-color,border-color,box-shadow,transform]",
    );
    expect(button.className).not.toMatch(/(?:^|\s)transition-all(?:\s|$)/);
    // the press-feedback utility itself is untouched by this plan.
    expect(button.className).toContain("active:not-aria-[haspopup]:translate-y-px");
  });
});

// plan 129 (T8, deep-review fix): Badge (unlike Button above) never
// carried a broad `transition-all` wildcard to begin with — this is a
// one-line regression guard, next to the Button pin above since they're
// the same class of finding, not a claim that Badge was ever touched by
// plan 126 itself.
it("Badge keeps the narrow transition-colors utility, never the transition-all wildcard", () => {
  render(<Badge>New</Badge>);
  const badge = screen.getByText("New");
  expect(badge.className).toContain("transition-colors");
  expect(badge.className).not.toMatch(/(?:^|\s)transition-all(?:\s|$)/);
});
