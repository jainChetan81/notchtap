export function mustQuery<T extends Element>(
  root: ParentNode,
  selector: string,
): T {
  const el = root.querySelector(selector);
  if (el === null) {
    throw new Error(`mustQuery: selector not found: ${selector}`);
  }
  return el as T;
}

export function mustClosest<T extends Element>(
  el: Element,
  selector: string,
): T {
  const closest = el.closest(selector);
  if (closest === null) {
    throw new Error(`mustClosest: selector not found: ${selector}`);
  }
  return closest as T;
}

export function mustBe<T>(value: T | null | undefined, message = "mustBe: value is nullish"): T {
  if (value === null || value === undefined) {
    throw new Error(message);
  }
  return value;
}
