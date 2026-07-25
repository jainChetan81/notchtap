// S3 consistency fix: the settings window had two of its own time
// regimes, on top of the overlay's — History's "Published" label used a
// hand-rolled 24-hour `HH:MM` (getHours/getMinutes + padStart), and each
// history row's own recorded-at timestamp used locale-floating
// `toLocaleString()`, which renders a different shape (and 12h/24h
// choice) depending on the host machine's locale. The overlay clock
// (`src/useClock.ts`) is operator-locked to a pinned en-US 12-hour
// format ("10:59 PM") specifically so every machine renders the same
// shape — this file reuses that exact `Intl.DateTimeFormat` shape as the
// one time-of-day formatter for both settings call sites, so nothing in
// the app still floats with system locale.
const TIME_FORMATTER = new Intl.DateTimeFormat("en-US", {
  hour: "numeric",
  minute: "2-digit",
  hour12: true,
});

// Row timestamps (unlike "Published", which is always shown inside a
// single already-dated context) can span many days of recorded history,
// so this pairs the SAME pinned time-of-day formatting above with an
// explicit date — still en-US, still deterministic, never the host
// locale's own date shape.
const DATE_TIME_FORMATTER = new Intl.DateTimeFormat("en-US", {
  year: "numeric",
  month: "short",
  day: "numeric",
  hour: "numeric",
  minute: "2-digit",
  hour12: true,
});

/** e.g. "10:59 PM" — byte-identical shape to the overlay clock's own format. */
export function formatClockTime(ms: number): string {
  return TIME_FORMATTER.format(new Date(ms));
}

/** e.g. "Nov 14, 2023, 10:13 PM" — same pinned time-of-day format as
 *  `formatClockTime`, with a date prefix for a value that can be days old. */
export function formatRecordedAt(ms: number): string {
  return DATE_TIME_FORMATTER.format(new Date(ms));
}
