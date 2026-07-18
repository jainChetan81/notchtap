import { describe, expect, it } from "vitest";
import {
  ageLabel,
  categoryClass,
  publishedLabel,
  sourceLabelFor,
  stampFor,
  tierCode,
  tierLabel,
} from "./presentation";

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
    expect(stampFor("high", "goal", "score_update")).toBe("Live");
    expect(stampFor("low", "goal", "news_item")).toBe("Live"); // signal wins over event type here
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

describe("sourceLabelFor", () => {
  it("labels generic events as cmux/CLI", () => {
    expect(sourceLabelFor("generic")).toBe("cmux / CLI · local");
  });

  it("labels espn-derived event types as football", () => {
    expect(sourceLabelFor("score_update")).toBe("ESPN · football");
    expect(sourceLabelFor("match_state")).toBe("ESPN · football");
  });

  it("labels news items as the RSS news wire", () => {
    expect(sourceLabelFor("news_item")).toBe("RSS · news wire");
  });
});

describe("categoryClass", () => {
  it("maps every known category to its shader class", () => {
    expect(categoryClass("politics")).toBe("cat-politics");
    expect(categoryClass("tech")).toBe("cat-tech");
    expect(categoryClass("sports")).toBe("cat-sports");
    expect(categoryClass("business")).toBe("cat-business");
    expect(categoryClass("world")).toBe("cat-world");
  });

  it("falls back to neutral gray for null and unknown categories", () => {
    expect(categoryClass(null)).toBe("cat-generic");
    expect(categoryClass("science")).toBe("cat-generic");
  });
});

describe("ageLabel", () => {
  const NOW = 2_000_000_000_000;

  it("returns null without a published time", () => {
    expect(ageLabel(null, NOW)).toBeNull();
  });

  it("formats minute, hour, and day age bands", () => {
    expect(ageLabel(NOW - 59_999, NOW)).toBe("<1m ago");
    expect(ageLabel(NOW - 60_000, NOW)).toBe("1m ago");
    expect(ageLabel(NOW - 59 * 60_000, NOW)).toBe("59m ago");
    expect(ageLabel(NOW - 60 * 60_000, NOW)).toBe("1h ago");
    expect(ageLabel(NOW - 23 * 60 * 60_000, NOW)).toBe("23h ago");
    expect(ageLabel(NOW - 24 * 60 * 60_000, NOW)).toBe("1d ago");
    expect(ageLabel(NOW - 3 * 24 * 60 * 60_000, NOW)).toBe("3d ago");
  });
});

describe("publishedLabel", () => {
  it("formats a publication timestamp as local 24-hour time", () => {
    const published = new Date(2026, 6, 17, 14, 32).getTime();
    expect(publishedLabel(published, published + 60_000)).toBe("14:32");
  });

  it("returns null without a published timestamp", () => {
    expect(publishedLabel(null, Date.now())).toBeNull();
  });
});
