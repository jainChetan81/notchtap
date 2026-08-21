import { useClock } from "../hooks/useClock";

// The left flank's clock — shared verbatim between the idle and
// showing/exiting states (StatusRailCard renders this component at both
// call sites), so the timer pattern (useClock's 30s tick) lives in
// exactly one place. Its class is `.time-only`, the prototype's own name
// (`prototype/notch-states.html:63`): this is shell furniture that
// renders in every state, not idle-only rail chrome.
export function FlankClock() {
  const { display } = useClock();
  return <span className="time-only">{display}</span>;
}
