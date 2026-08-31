import { useEffect, useState } from "react";

const formatter = new Intl.DateTimeFormat("en-US", {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});

export type ClockReading = {
  display: string;
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

export function useClock(): ClockReading {
  const [reading, setReading] = useState(read);

  useEffect(() => {
    const id = window.setInterval(() => {
      setReading(read());
    }, 30_000);
    return () => window.clearInterval(id);
  }, []);

  return reading;
}
