import { describe, expect, it } from "vitest";

import {
  isDetailArray,
  isFunction,
  isNonNegativeInteger,
  isNullableString,
  isNumber,
  isPositiveFiniteNumber,
  isString,
} from "./guards";

describe("guards", () => {
  it("isString", () => {
    expect(isString("a")).toBe(true);
    expect(isString(1)).toBe(false);
  });
  it("isNumber", () => {
    expect(isNumber(1)).toBe(true);
    expect(isNumber("1")).toBe(false);
  });
  it("isFunction", () => {
    expect(isFunction(() => {})).toBe(true);
    expect(isFunction("x")).toBe(false);
  });
  it("isPositiveFiniteNumber", () => {
    expect(isPositiveFiniteNumber(1)).toBe(true);
    expect(isPositiveFiniteNumber(0)).toBe(false);
    expect(isPositiveFiniteNumber(Infinity)).toBe(false);
  });
  it("isNonNegativeInteger", () => {
    expect(isNonNegativeInteger(0)).toBe(true);
    expect(isNonNegativeInteger(-1)).toBe(false);
    expect(isNonNegativeInteger(1.5)).toBe(false);
  });
  it("isNullableString", () => {
    expect(isNullableString(null)).toBe(true);
    expect(isNullableString("x")).toBe(true);
    expect(isNullableString(1)).toBe(false);
  });
  it("isDetailArray", () => {
    expect(isDetailArray([{ label: "a", value: "b" }])).toBe(true);
    expect(isDetailArray([{ label: "a", value: 1 }])).toBe(false);
    expect(isDetailArray(null)).toBe(false);
  });
});
