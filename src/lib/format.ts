const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

const TIME_FORMATTER = new Intl.DateTimeFormat("en-US", {
  hour: "numeric",
  minute: "2-digit",
  hour12: true,
});

const DATE_TIME_FORMATTER = new Intl.DateTimeFormat("en-US", {
  year: "numeric",
  month: "short",
  day: "numeric",
  hour: "numeric",
  minute: "2-digit",
  hour12: true,
});

function unitExponent(bytes: number): number {
  if (bytes <= 0) return 0;
  return Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), UNITS.length - 1);
}

export function formatBytes(bytes: number | null): string {
  if (bytes === null || Number.isNaN(bytes)) return "—";
  if (bytes === 0) return "0 B";
  const exponent = unitExponent(bytes);
  const value = bytes / 1024 ** exponent;
  const digits = exponent === 0 ? 0 : 1;
  return `${value.toFixed(digits)} ${UNITS[exponent]}`;
}

export function formatBytePair(usedBytes: number | null, totalBytes: number | null): string {
  if (usedBytes === null || totalBytes === null) return "—";
  const exponent = unitExponent(Math.max(totalBytes, usedBytes));
  const digits = exponent === 0 ? 0 : 1;
  const scale = 1024 ** exponent;
  const used = (usedBytes / scale).toFixed(digits);
  const total = (totalBytes / scale).toFixed(digits);
  return `${used} / ${total} ${UNITS[exponent]}`;
}

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

export function formatClockTime(ms: number): string {
  return TIME_FORMATTER.format(new Date(ms));
}

export function formatRecordedAt(ms: number): string {
  return DATE_TIME_FORMATTER.format(new Date(ms));
}
