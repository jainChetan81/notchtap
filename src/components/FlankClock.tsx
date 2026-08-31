import { useClock } from "../hooks/useClock";

export function FlankClock() {
  const { display } = useClock();
  return <span className="time-only">{display}</span>;
}
