import { useEffect, useState } from "react";

// Idle clock: purely visual, local to the webview — never touches
// queue/Event/Priority. en-US + hour12:false pinned: every machine
// renders the same two-digit 24h shape (notch width is fixed).
const formatter = new Intl.DateTimeFormat("en-US", {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});

export type ClockReading = {
  display: string;
  // 0-100: progress through the local day (idle day-progress dot).
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

// Owned by <IdleView> alone: only ticks while idle, never rerenders
// the caller during showing-state.
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
