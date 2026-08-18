import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Badge } from "./badge";
import { Button } from "./button";

// this project's vitest config doesn't set `test.globals`, so RTL's
// auto-cleanup (which hooks a global `afterEach`) never registers.
afterEach(cleanup);

// The button carries an explicit transition-property list, never the
// bare `transition-all` wildcard (which animates every property change,
// including ones no author intended). That list must keep `translate`
// and `scale` — they are what make `active:not-aria-[haspopup]:
// translate-y-px` and `active:scale-[0.96]`'s press feedback animate
// instead of snapping. Tailwind v4 emits `scale-*`/`translate-*` as the
// standalone `scale`/`translate` CSS properties, NOT as `transform`, so
// the pin names those two and never `transform`. String-level pin, not a
// computed-style assertion: jsdom doesn't run CSS transitions, so the
// only thing to assert is the utility class's own property list.
describe("Button — transition-property", () => {
  it("keeps translate/scale in the transition property list, and drops the bare transition-all wildcard", () => {
    render(<Button>Click me</Button>);
    const button = screen.getByRole("button", { name: "Click me" });

    expect(button.className).toContain(
      "transition-[color,background-color,border-color,box-shadow,translate,scale]",
    );
    expect(button.className).not.toMatch(/(?:^|\s)transition-all(?:\s|$)/);
    // the press-feedback utility itself is untouched by this plan.
    expect(button.className).toContain("active:not-aria-[haspopup]:translate-y-px");
  });
});

// Badge (unlike Button above) never
// carried a broad `transition-all` wildcard to begin with — this is a
// one-line regression guard, next to the Button pin above since they're
// the same class of finding, not a claim that Badge was ever touched by
// itself.
it("Badge keeps the narrow transition-colors utility, never the transition-all wildcard", () => {
  render(<Badge>New</Badge>);
  const badge = screen.getByText("New");
  expect(badge.className).toContain("transition-colors");
  expect(badge.className).not.toMatch(/(?:^|\s)transition-all(?:\s|$)/);
});
