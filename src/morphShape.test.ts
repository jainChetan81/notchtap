import { describe, it, expect } from "vitest";
import { getMorphShape } from "./morphShape";

describe("getMorphShape", () => {
  it("maps score_update to pill", () => {
    expect(getMorphShape("score_update")).toBe("pill");
  });

  it("maps match_state to pill", () => {
    expect(getMorphShape("match_state")).toBe("pill");
  });

  it("maps generic to grow", () => {
    expect(getMorphShape("generic")).toBe("grow");
  });

  it("falls back unknown types to grow", () => {
    expect(getMorphShape("totally_unknown_type")).toBe("grow");
  });
});
