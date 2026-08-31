import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { IconStrip, type IconStripProps } from "./IconStrip";

afterEach(cleanup);

const BASE: IconStripProps = {
	agent: "hidden",
	football: "hidden",
	news: "present",
	newsCharge: 0,
	newsCharged: false,
	newsCount: null,
	selected: null
};

describe("IconStrip", () => {
	it("a hidden icon carries no is-present/is-live class and is disabled", () => {
		const { container } = render(<IconStrip {...BASE} agent="hidden" />);
		// SAFETY: IconStrip always renders all three tabs regardless of presence —
		// even agent="hidden" mounts the `.icon.agent` glyph.
		const agentIcon = container.querySelector(".icon.agent") as HTMLButtonElement;
		expect(agentIcon.classList.contains("is-present")).toBe(false);
		expect(agentIcon.classList.contains("is-live")).toBe(false);
		expect(agentIcon.disabled).toBe(true);
	});

	it("marks the selected tab is-selected and aria-pressed, and only that one", () => {
		const { container } = render(<IconStrip {...BASE} agent="live" selected="agent" />);
		const agentIcon = container.querySelector(".icon.agent");
		const newsIcon = container.querySelector(".icon.news");
		expect(agentIcon?.classList.contains("is-selected")).toBe(true);
		expect(agentIcon?.getAttribute("aria-pressed")).toBe("true");
		expect(newsIcon?.classList.contains("is-selected")).toBe(false);
		expect(newsIcon?.getAttribute("aria-pressed")).toBe("false");
	});

	it("clicking a present icon fires onSelect with that tab", () => {
		const onSelect = vi.fn();
		render(<IconStrip {...BASE} news="present" onSelect={onSelect} />);
		fireEvent.click(screen.getByRole("button", { name: "News" }));
		expect(onSelect).toHaveBeenCalledWith("news");
		expect(onSelect).toHaveBeenCalledTimes(1);
	});

	it("news charge fill scaleY reflects newsCharge, clamped to [0,1]", () => {
		const { container: mid } = render(<IconStrip {...BASE} newsCharge={0.4} />);
		// SAFETY: the `.charge` rect always renders inside the news glyph, so a
		// mid-charge fixture's match is a real SVGElement.
		const midFill = mid.querySelector(".icon.news .charge") as SVGElement;
		expect(midFill.style.transform).toBe("scaleY(0.4)");

		const { container: over } = render(<IconStrip {...BASE} newsCharge={1.5} />);
		// SAFETY: the `.charge` rect always renders inside the news glyph, so an
		// over-100% fixture's match is a real SVGElement.
		const overFill = over.querySelector(".icon.news .charge") as SVGElement;
		expect(overFill.style.transform).toBe("scaleY(1)");

		const { container: under } = render(<IconStrip {...BASE} newsCharge={-0.3} />);
		// SAFETY: the `.charge` rect always renders inside the news glyph, so a
		// negative-charge fixture's match is a real SVGElement.
		const underFill = under.querySelector(".icon.news .charge") as SVGElement;
		expect(underFill.style.transform).toBe("scaleY(0)");
	});

	it("two IconStrips in the same document never collide on the news glyph's clip-path id", () => {
		const { container } = render(
			<div>
				<IconStrip {...BASE} />
				<IconStrip {...BASE} />
			</div>
		);
		const clipIds = Array.from(container.querySelectorAll("clipPath")).map((el) => el.id);
		expect(clipIds).toHaveLength(2);
		expect(new Set(clipIds).size).toBe(2);
		const fills = container.querySelectorAll(".icon.news .charge");
		expect(fills[0].getAttribute("clip-path")).toBe(`url(#${clipIds[0]})`);
		expect(fills[1].getAttribute("clip-path")).toBe(`url(#${clipIds[1]})`);
	});

	it("every present icon has an accessible name matching its tab", () => {
		render(<IconStrip {...BASE} agent="live" football="live" news="present" />);
		for (const name of ["Agent", "Football", "News"]) {
			expect(screen.getByRole("button", { name })).toBeTruthy();
		}
	});

	it("includes the pending count in the news tab's accessible name, since an aria-label overrides the visible badge for assistive tech", () => {
		render(<IconStrip {...BASE} news="present" newsCount={3} />);
		expect(screen.getByRole("button", { name: "News, 3 new" })).toBeTruthy();
		expect(screen.queryByRole("button", { name: "News" })).toBeNull();
	});
});
