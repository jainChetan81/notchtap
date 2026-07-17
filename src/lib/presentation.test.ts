import { describe, it, expect } from "vitest";
import { tierCode, tierLabel, stampFor, sourceLabelFor } from "./presentation";

describe("tierCode", () => {
  it("maps each priority to its short code", () => {
    expect(tierCode("low")).toBe("L1");
    expect(tierCode("medium")).toBe("M2");
    expect(tierCode("high")).toBe("H3");
  });
});

describe("tierLabel", () => {
  it("maps each priority to its full label", () => {
    expect(tierLabel("low")).toBe("Low");
    expect(tierLabel("medium")).toBe("Medium");
    expect(tierLabel("high")).toBe("High");
  });
});

describe("stampFor", () => {
  it("uses the fixed per-signal table when signal is not generic, regardless of priority", () => {
    expect(stampFor("high", "goal")).toBe("Live");
    expect(stampFor("low", "goal")).toBe("Live"); // signal wins over priority here
    expect(stampFor("medium", "halftime")).toBe("Break");
    expect(stampFor("medium", "yellow_card")).toBe("Card");
    expect(stampFor("medium", "fulltime")).toBe("Final");
    expect(stampFor("high", "red_card")).toBe("Off");
    expect(stampFor("low", "kickoff")).toBe("Live");
  });

  it("falls back to the priority-derived table when signal is generic", () => {
    expect(stampFor("low", "generic")).toBe("Live");
    expect(stampFor("medium", "generic")).toBe("Done");
    expect(stampFor("high", "generic")).toBe("Now");
  });
});

describe("sourceLabelFor", () => {
  it("labels generic events as cmux/CLI", () => {
    expect(sourceLabelFor("generic")).toBe("cmux / CLI · local");
  });

  it("labels espn-derived event types as football", () => {
    expect(sourceLabelFor("score_update")).toBe("ESPN · football");
    expect(sourceLabelFor("match_state")).toBe("ESPN · football");
  });
});
