import { describe, expect, it } from "vitest";
import { formatBytePair, formatBytes, formatUptime } from "./byteFormat";

describe("formatBytes", () => {
  it("formats null as an em dash", () => {
    expect(formatBytes(null)).toBe("—");
  });

  it("formats zero explicitly", () => {
    expect(formatBytes(0)).toBe("0 B");
  });

  it("formats sub-kilobyte values in bytes with no decimal", () => {
    expect(formatBytes(930)).toBe("930 B");
  });

  it("formats megabytes with one decimal place", () => {
    expect(formatBytes(48_200_000)).toBe("46.0 MB");
  });

  it("formats gigabytes with one decimal place", () => {
    expect(formatBytes(12_400_000_000)).toBe("11.5 GB");
  });

  it("formats terabytes", () => {
    expect(formatBytes(1_099_511_627_776)).toBe("1.0 TB");
  });
});

describe("formatBytePair", () => {
  it("returns an em dash when either side is null", () => {
    expect(formatBytePair(null, 100)).toBe("—");
    expect(formatBytePair(100, null)).toBe("—");
    expect(formatBytePair(null, null)).toBe("—");
  });

  it("scales both values to the unit derived from the total", () => {
    // 12.4 GB used / 16 GB total (approx, in binary GB)
    expect(formatBytePair(13_300_000_000, 17_179_869_184)).toBe("12.4 / 16.0 GB");
  });

  it("stays in bytes (no decimals) when the total is sub-kilobyte", () => {
    expect(formatBytePair(0, 512)).toBe("0 / 512 B");
  });
});

describe("formatUptime", () => {
  it("shows seconds only under a minute", () => {
    expect(formatUptime(8)).toBe("8s");
  });

  it("shows minutes and seconds under an hour", () => {
    expect(formatUptime(65)).toBe("1m 5s");
  });

  it("shows hours and minutes under a day", () => {
    expect(formatUptime(3 * 3600 + 12 * 60)).toBe("3h 12m");
  });

  it("shows days and hours at a day or more", () => {
    expect(formatUptime(2 * 86400 + 4 * 3600)).toBe("2d 4h");
  });

  it("clamps negative/NaN input to 0s", () => {
    expect(formatUptime(-5)).toBe("0s");
    expect(formatUptime(Number.NaN)).toBe("0s");
  });
});
