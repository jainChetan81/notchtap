export function isString(v: unknown): v is string {
  return typeof v === "string";
}

export function isNumber(v: unknown): v is number {
  return typeof v === "number";
}

export function isFunction(v: unknown): v is (...args: never[]) => void {
  return typeof v === "function";
}

export function isPositiveFiniteNumber(v: unknown): v is number {
  return typeof v === "number" && Number.isFinite(v) && v > 0;
}

export function isNonNegativeInteger(v: unknown): v is number {
  return typeof v === "number" && Number.isInteger(v) && v >= 0;
}

export function isNullableString(v: unknown): v is string | null {
  return v === null || typeof v === "string";
}

export function isDetailArray(v: unknown): v is { label: string; value: string }[] {
  return (
    Array.isArray(v) &&
    v.every(
      (item) =>
        typeof item === "object" &&
        item !== null &&
        "label" in item &&
        "value" in item &&
        // SAFETY: "label" in item is the runtime check that item has a label property — the cast only narrows the property access.
        typeof (item as { label: unknown }).label === "string" &&
        // SAFETY: "value" in item is the runtime check that item has a value property — the cast only narrows the property access.
        typeof (item as { value: unknown }).value === "string",
    )
  );
}
