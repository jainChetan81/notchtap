import { useEffect, useState } from "react";

// idle-state fallback (grilled 2026-07-17): purely visual, local to the
// webview — never touches SingleSlotQueue/Event/Priority. computes "now"
// directly rather than being pushed data, so no backend plumbing exists
// for it at all.
// plan 091: `display` is now the flank clock's HH:MM text — the prototype's
// locked idle rail (`prototype/notch-states.html:220`, `<span
// class="time-only">14:32</span>`) is 24h, no date. `hourCycle: "h23"`
// forces 0-23 regardless of the user's locale (some locales' default
// 24h cycle uses "24" for midnight instead of "00", which `time-only`'s
// reference never shows).
const formatter = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
});

export type ClockReading = {
  display: string;
  // 0-100, how far through the local day "now" is — the idle view's
  // day-progress timeline dot, ported from the status-rail prototype.
  dayProgress: number;
};

function read(): ClockReading {
  const now = new Date();
  const minutesIntoDay = now.getHours() * 60 + now.getMinutes();
  return {
    display: formatter.format(now),
    dayProgress: (minutesIntoDay / 1440) * 100,
  };
}

// Deliberately owned by <IdleView> alone (review finding: this hook was
// ticking, and rerendering its caller, even while a notification was
// showing — a 30s timer that only ever matters during idle has no
// business firing during showing-state renders).
export function useClock(): ClockReading {
  const [reading, setReading] = useState(read);

  useEffect(() => {
    // display has no seconds, so a 30s tick is plenty — catches every
    // minute boundary within half a minute without re-rendering every tick
    const id = window.setInterval(() => {
      setReading(read());
    }, 30_000);
    return () => window.clearInterval(id);
  }, []);

  return reading;
}
