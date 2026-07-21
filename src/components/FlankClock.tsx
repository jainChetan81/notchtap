import { useClock } from "../useClock";

// plan 091: the left flank's clock — shared verbatim between the idle and
// showing/exiting states (StatusRailCard renders this component at both
// call sites), extracted from the old IdleView's `<span className="time">`
// so the timer pattern (useClock's 30s tick) lives in exactly one place.
// `.time-only` is the prototype's own class name
// (`prototype/notch-states.html:63`), not `.time` (the old idle-only rail's
// class) — deliberately renamed since this is now shell furniture that
// renders in states the old `.idle-view .time` selector never covered.
export function FlankClock() {
  const { display } = useClock();
  return <span className="time-only">{display}</span>;
}
