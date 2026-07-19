// Shared tauri event-channel mock (plan 028): one harness for every
// test that feeds the overlay listeners. Handlers are kept per event
// name — the previous per-file copies lumped all listeners into one
// array and emitted slot-state payloads at every registered handler.
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
