import { describe, expect, it } from "vitest";
import type { SlotState } from "../useSlotState";
import { presentationMode, stampFor } from "./presentation";

describe("stampFor", () => {
	it("uses the fixed per-signal table when signal is not generic, regardless of priority", () => {
		expect(stampFor("high", "goal", "score_update")).toBe("Live");
		expect(stampFor("low", "goal", "news_item")).toBe("Live");
		expect(stampFor("medium", "halftime", "match_state")).toBe("Break");
		expect(stampFor("medium", "yellow_card", "match_state")).toBe("Card");
		expect(stampFor("medium", "fulltime", "match_state")).toBe("Final");
		expect(stampFor("high", "red_card", "match_state")).toBe("Off");
		expect(stampFor("low", "kickoff", "match_state")).toBe("Live");
	});

	it("falls back to the priority-derived table when signal is generic", () => {
		expect(stampFor("low", "generic", "generic")).toBe("Live");
		expect(stampFor("medium", "generic", "score_update")).toBe("Done");
		expect(stampFor("high", "generic", "match_state")).toBe("Now");
	});

	it("uses Wire for a generic news signal", () => {
		expect(stampFor("low", "generic", "news_item")).toBe("Wire");
	});
});

describe("presentationMode", () => {
	const empty: SlotState = { state: "empty" };
	const showing: SlotState = {
		state: "showing",
		id: "n1",
		title: "t",
		body: "b",
		eventType: "generic",
		priority: "medium",
		signal: "generic",
		origin: "manual",
		agentRuntime: null,
		expanded: false,
		source: null,
		category: null,
		publishedAtMs: null,
		link: null,
		subtitle: null,
		details: [],
		queueTotal: 1,
		queueDone: 0,
		ttlMs: 8000,
		remainingMs: 8000
	};

	it("a Visible Notification always wins, regardless of session count", () => {
		expect(presentationMode(showing, 0, false)).toBe("notification");
		expect(presentationMode(showing, 3, false)).toBe("notification");
	});

	it("shows the board when the slot is empty and at least one session exists", () => {
		expect(presentationMode(empty, 1, false)).toBe("board");
		expect(presentationMode(empty, 5, false)).toBe("board");
	});

	it("falls back to idle when the slot is empty and no session exists", () => {
		expect(presentationMode(empty, 0, false)).toBe("idle");
	});

	it("hides the board while paused, falling through to idle even with sessions present", () => {
		expect(presentationMode(empty, 1, true)).toBe("idle");
		expect(presentationMode(empty, 5, true)).toBe("idle");
	});

	it("still idles while paused with no sessions at all", () => {
		expect(presentationMode(empty, 0, true)).toBe("idle");
	});

	it("leaves a Visible Notification's precedence untouched while paused", () => {
		expect(presentationMode(showing, 0, true)).toBe("notification");
		expect(presentationMode(showing, 3, true)).toBe("notification");
	});
});
