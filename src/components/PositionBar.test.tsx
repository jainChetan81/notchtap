import { describe, expect, it } from "vitest";
import { segmentFor } from "./PositionBar";

describe("segmentFor", () => {
	it("maps 1:1 when total is at or under the 10-segment ceiling", () => {
		expect(segmentFor(2, 5)).toEqual({ segmentCount: 5, segmentIndex: 2 });
	});

	it("caps the segment count at 10 for larger totals", () => {
		expect(segmentFor(0, 15).segmentCount).toBe(10);
	});

	it("maps the current index proportionally past the ceiling", () => {
		expect(segmentFor(10, 20)).toEqual({ segmentCount: 10, segmentIndex: 5 });
		expect(segmentFor(19, 20)).toEqual({ segmentCount: 10, segmentIndex: 9 });
	});

	it("clamps a negative or out-of-range index into the segment span", () => {
		expect(segmentFor(-1, 5).segmentIndex).toBe(0);
		expect(segmentFor(99, 5).segmentIndex).toBe(4);
	});

	it("treats a non-positive total as a single segment", () => {
		expect(segmentFor(0, 0)).toEqual({ segmentCount: 1, segmentIndex: 0 });
	});
});
