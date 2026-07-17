import { stampFor, type Priority, type EventSignal } from "../lib/presentation";

export function Stamp({ priority, signal }: { priority: Priority; signal: EventSignal }) {
  return <div className="stamp">{stampFor(priority, signal)}</div>;
}
