// Human-formatting helpers for the About section's system card
// (AboutSection.tsx). Pulled into their own tiny, dependency-free module
// so they're unit-testable without mounting the section — the same
// reasoning as about.rs's pure functions on the rust side.

const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

function unitExponent(bytes: number): number {
  if (bytes <= 0) return 0;
  return Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), UNITS.length - 1);
}

/** "48.2 MB", "930 B", "1.0 TB". `null` (best-effort field unavailable) -> "—". */
export function formatBytes(bytes: number | null): string {
  if (bytes === null || Number.isNaN(bytes)) return "—";
  if (bytes === 0) return "0 B";
  const exponent = unitExponent(bytes);
  const value = bytes / 1024 ** exponent;
  const digits = exponent === 0 ? 0 : 1;
  return `${value.toFixed(digits)} ${UNITS[exponent]}`;
}

/** "12.4 / 16 GB" — both values scaled to the same unit (derived from
 * `totalBytes`, always the larger of the two) so the pair reads as one
 * number, not two independently-rounded ones. Either side `null` -> "—". */
export function formatBytePair(usedBytes: number | null, totalBytes: number | null): string {
  if (usedBytes === null || totalBytes === null) return "—";
  const exponent = unitExponent(Math.max(totalBytes, usedBytes));
  const digits = exponent === 0 ? 0 : 1;
  const scale = 1024 ** exponent;
  const used = (usedBytes / scale).toFixed(digits);
  const total = (totalBytes / scale).toFixed(digits);
  return `${used} / ${total} ${UNITS[exponent]}`;
}

/** "2d 4h", "3h 12m", "45m 10s", "8s" — always the top two non-zero
 * units, never more (an uptime display doesn't need seconds once it's
 * been running for days). Negative/NaN input clamps to 0s rather than
 * throwing — this only ever feeds from a monotonic process uptime, but a
 * defensive floor costs nothing. */
export function formatUptime(totalSeconds: number): string {
  const seconds = Number.isFinite(totalSeconds) ? Math.max(0, Math.floor(totalSeconds)) : 0;
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${secs}s`;
  return `${secs}s`;
}
