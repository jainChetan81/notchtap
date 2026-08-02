import { useEffect, useState } from "react";

// idle-state fallback (grilled 2026-07-17): purely visual, local to the
// webview — never touches SingleSlotQueue/Event/Priority. computes "now"
// directly rather than being pushed data, so no backend plumbing exists
// for it at all.
// plan 091 locked this to the prototype's 24h "14:32"; operator decision
// 2026-07-24 briefly overrode that to 12-hour AM/PM; operator decision
// 2026-08-02 reverts to 24h — the fixed notch width clips the "M" of
// "AM"/"PM" in the small idle state, and the width is not negotiable.
// Locale pinned to en-US so every machine renders the same shape
// (two-digit 24h clock) instead of drifting with system conventions.
const formatter = new Intl.DateTimeFormat("en-US", {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
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
