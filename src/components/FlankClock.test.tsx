import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { FlankClock } from "./FlankClock";

// FlankClock's digit-roll reads `hours`/`minutes` off `useClock` — mocked
// here so each test drives an exact reading instead of the real 30s-tick
// hook (real-clock timing has no place in a unit test asserting on digit
// formatting/updates). Vitest hoists `vi.mock` above the imports above at
// transform time regardless of source order, so this still applies to the
// `FlankClock` import.
const mockUseClock = vi.fn();
vi.mock("../useClock", () => ({
  useClock: () => mockUseClock(),
}));

afterEach(cleanup);

// NumberFlow (FlankClock's hour/minute digits) renders inside a shadow
// root — and jsdom applies no CSS at all, so a naive
// `shadowRoot.textContent` read picks up every candidate digit 0-9 the
// library keeps in the DOM for its roll animation (concealed only by a
// `[inert] { display: none }` rule jsdom never applies), plus the
// injected `<style>` text itself. The element's own `_data.valueAsString`
// — a plain, non-private instance field NumberFlow sets from the exact
// formatted string it renders (`number-flow/lite`'s `set data()`) — is
// the reliable read instead. This walks the wrapper in document order,
// using that field for any `number-flow-react` host and light-DOM
// textContent for everything else, reproducing what a user actually sees
// on screen.
function renderedText(el: Element | null): string {
  if (el === null) {
    return "";
  }
  let out = "";
  for (const node of Array.from(el.childNodes)) {
    if (node.nodeType === Node.TEXT_NODE) {
      out += node.textContent ?? "";
    } else if (node instanceof Element) {
      if (node.tagName.toLowerCase() === "number-flow-react") {
        const data = (node as unknown as { _data?: { valueAsString?: string } })._data;
        out += data?.valueAsString ?? "";
      } else {
        out += renderedText(node);
      }
    }
  }
  return out;
}

describe("FlankClock", () => {
  it("renders zero-padded HH:MM from useClock's numeric hours/minutes", () => {
    mockUseClock.mockReturnValue({ display: "09:05", hours: 9, minutes: 5, dayProgress: 37.8 });
    const { container } = render(<FlankClock />);
    const clock = container.querySelector(".time-only");
    expect(clock).not.toBeNull();
    expect(renderedText(clock)).toBe("09:05");
  });

  it("reflects a minute tick without remounting (a same-tree update, not a re-parse of `display`)", () => {
    mockUseClock.mockReturnValue({ display: "14:32", hours: 14, minutes: 32, dayProgress: 60.5 });
    const { container, rerender } = render(<FlankClock />);
    expect(renderedText(container.querySelector(".time-only"))).toBe("14:32");

    mockUseClock.mockReturnValue({ display: "14:33", hours: 14, minutes: 33, dayProgress: 60.6 });
    rerender(<FlankClock />);
    expect(renderedText(container.querySelector(".time-only"))).toBe("14:33");
  });

  it("rolls the hour digit too, at a fresh-hour boundary (00:00 midnight)", () => {
    mockUseClock.mockReturnValue({ display: "00:00", hours: 0, minutes: 0, dayProgress: 0 });
    const { container } = render(<FlankClock />);
    expect(renderedText(container.querySelector(".time-only"))).toBe("00:00");
  });
});
