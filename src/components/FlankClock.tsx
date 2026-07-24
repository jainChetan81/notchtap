import NumberFlow from "@number-flow/react";
import { useClock } from "../useClock";

// plan 091: the left flank's clock — shared verbatim between the idle and
// showing/exiting states (StatusRailCard renders this component at both
// call sites), extracted from the old IdleView's `<span className="time">`
// so the timer pattern (useClock's 30s tick) lives in exactly one place.
// `.time-only` is the prototype's own class name
// (`prototype/notch-states.html:63`), not `.time` (the old idle-only rail's
// class) — deliberately renamed since this is now shell furniture that
// renders in states the old `.idle-view .time` selector never covered.
//
// NumberFlow digit-roll (bonus item, always-on so it must stay cheap):
// hours/minutes render as two separate `NumberFlow`s around a static `:`
// — split off `useClock`'s own numeric `hours`/`minutes` fields rather
// than re-parsing `display`, so there's exactly one source of "what time
// is it" (this hook's `read()`) feeding both the old formatted string (now
// unused here, still available to any other caller) and these digits.
// `minimumIntegerDigits: 2` reproduces the old formatter's zero-padding
// ("07", not "7"). No custom `transformTiming`/`spinTiming` — NumberFlow's
// default spring is left untouched, and its own default behavior already
// covers both requirements the 30s tick needs: it does not animate the
// very first render (a value is only ever animated on a REACT UPDATE —
// `NumberFlowImpl.getSnapshotBeforeUpdate` in `@number-flow/react` only
// fires `willUpdate`/`didUpdate` when `prevProps.data !== props.data`,
// never from `componentDidMount`), and a same-value tick (the 30s
// interval firing without the minute actually changing) never touches the
// DOM at all since `set data` on the custom element no-ops when the
// incoming value matches what's already rendered.
export function FlankClock() {
  const { hours, minutes } = useClock();
  return (
    <span className="time-only">
      <NumberFlow value={hours} format={{ minimumIntegerDigits: 2 }} />
      <span className="clock-colon">:</span>
      <NumberFlow value={minutes} format={{ minimumIntegerDigits: 2 }} />
    </span>
  );
}
