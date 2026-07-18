import { type EventSignal, type EventType, type Priority, stampFor } from "../lib/presentation";

export function Stamp({
  priority,
  signal,
  eventType,
}: {
  priority: Priority;
  signal: EventSignal;
  eventType: EventType;
}) {
  return <div className="stamp">{stampFor(priority, signal, eventType)}</div>;
}
