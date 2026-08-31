import { vi } from "vitest";

type Handler = (event: { payload: unknown }) => void;
const handlersByName = new Map<string, Handler[]>();

export const listen = vi.fn((name: string, handler: Handler) => {
  const list = handlersByName.get(name) ?? [];
  list.push(handler);
  handlersByName.set(name, list);
  return Promise.resolve(() => {});
});

export function emitTo(name: string, payload: unknown) {
  for (const handler of handlersByName.get(name) ?? []) {
    handler({ payload });
  }
}

export function resetHandlers() {
  handlersByName.clear();
}
