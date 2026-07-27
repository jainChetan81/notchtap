import { describe, expect, it } from "vitest";
import type { AgentRuntime } from "../useAgentState";
import type { SlotState } from "../useSlotState";
import {
  ageLabel,
  agentRuntimeClass,
  agentRuntimeLabel,
  agentStatePresentationFor,
  categoryClass,
  elapsedLabel,
  presentationMode,
  sourceClass,
  stampFor,
} from "./presentation";

describe("stampFor", () => {
  it("uses the fixed per-signal table when signal is not generic, regardless of priority", () => {
    expect(stampFor("high", "goal", "score_update")).toBe("Live");
    expect(stampFor("low", "goal", "news_item")).toBe("Live"); // signal wins over event type here
    expect(stampFor("medium", "halftime", "match_state")).toBe("Break");
    expect(stampFor("medium", "yellow_card", "match_state")).toBe("Card");
    expect(stampFor("medium", "fulltime", "match_state")).toBe("Final");
    expect(stampFor("high", "red_card", "match_state")).toBe("Off");
    expect(stampFor("low", "kickoff", "match_state")).toBe("Live");
  });

  it("falls back to the priority-derived table when signal is generic", () => {
    expect(stampFor("low", "generic", "generic")).toBe("Live");
    expect(stampFor("medium", "generic", "score_update")).toBe("Done");
    expect(stampFor("high", "generic", "match_state")).toBe("Now");
  });

  it("uses Wire for a generic news signal", () => {
    expect(stampFor("low", "generic", "news_item")).toBe("Wire");
  });
});

describe("categoryClass", () => {
  it("maps every known category to its shader class", () => {
    expect(categoryClass("politics")).toBe("cat-politics");
    expect(categoryClass("tech")).toBe("cat-tech");
    expect(categoryClass("sports")).toBe("cat-sports");
    expect(categoryClass("business")).toBe("cat-business");
    expect(categoryClass("world")).toBe("cat-world");
    expect(categoryClass("science")).toBe("cat-science");
  });

  it("falls back to neutral gray for null and unknown categories", () => {
    expect(categoryClass(null)).toBe("cat-generic");
    expect(categoryClass("astrology")).toBe("cat-generic");
  });
});

describe("sourceClass", () => {
  const RUNTIMES: AgentRuntime[] = ["claude-code", "codex", "kimi", "opencode"];

  it("maps every agent runtime to its own identity class", () => {
    expect(sourceClass("agent", "claude-code")).toBe("src-claude-code");
    expect(sourceClass("agent", "codex")).toBe("src-codex");
    expect(sourceClass("agent", "kimi")).toBe("src-kimi");
    expect(sourceClass("agent", "opencode")).toBe("src-opencode");
  });

  it("falls back to the neutral agent class when runtime is unknown", () => {
    expect(sourceClass("agent", null)).toBe("src-agent");
  });

  it("maps every non-agent, non-news origin to its own identity class", () => {
    expect(sourceClass("football", null)).toBe("src-football");
    expect(sourceClass("weather", null)).toBe("src-weather");
    expect(sourceClass("manual", null)).toBe("src-manual");
  });

  it("agentRuntimeClass is total over every runtime (mirrors AGENT_RUNTIME_CLASS)", () => {
    for (const runtime of RUNTIMES) {
      expect(agentRuntimeClass(runtime)).toBe(sourceClass("agent", runtime));
    }
  });
});

describe("ageLabel", () => {
  const NOW = 2_000_000_000_000;

  it("returns null without a published time", () => {
    expect(ageLabel(null, NOW)).toBeNull();
  });

  it("formats minute, hour, and day age bands", () => {
    expect(ageLabel(NOW - 59_999, NOW)).toBe("<1m ago");
    expect(ageLabel(NOW - 60_000, NOW)).toBe("1m ago");
    expect(ageLabel(NOW - 59 * 60_000, NOW)).toBe("59m ago");
    expect(ageLabel(NOW - 60 * 60_000, NOW)).toBe("1h ago");
    expect(ageLabel(NOW - 23 * 60 * 60_000, NOW)).toBe("23h ago");
    expect(ageLabel(NOW - 24 * 60 * 60_000, NOW)).toBe("1d ago");
    expect(ageLabel(NOW - 3 * 24 * 60 * 60_000, NOW)).toBe("3d ago");
  });
});

// Plan 136 (v7 ticket 4 of 13, spec §6.1): the presentation precedence
// machine's own unit coverage — a Visible Notification always wins;
// otherwise a non-empty Agent Board session count shows the board;
// otherwise idle.
describe("presentationMode", () => {
  const empty: SlotState = { state: "empty" };
  const showing: SlotState = {
    state: "showing",
    id: "n1",
    title: "t",
    body: "b",
    eventType: "generic",
    priority: "medium",
    signal: "generic",
    origin: "manual",
    agentRuntime: null,
    expanded: false,
    source: null,
    category: null,
    publishedAtMs: null,
    link: null,
    subtitle: null,
    details: [],
    queueTotal: 1,
    queueDone: 0,
    ttlMs: 8000,
    remainingMs: 8000,
  };

  it("a Visible Notification always wins, regardless of session count", () => {
    expect(presentationMode(showing, 0)).toBe("notification");
    expect(presentationMode(showing, 3)).toBe("notification");
  });

  it("shows the board when the slot is empty and at least one session exists", () => {
    expect(presentationMode(empty, 1)).toBe("board");
    expect(presentationMode(empty, 5)).toBe("board");
  });

  it("falls back to idle when the slot is empty and no session exists", () => {
    expect(presentationMode(empty, 0)).toBe("idle");
  });
});

describe("elapsedLabel", () => {
  it("formats seconds under a minute", () => {
    expect(elapsedLabel(0)).toBe("0s");
    expect(elapsedLabel(45_000)).toBe("45s");
    expect(elapsedLabel(59_999)).toBe("59s");
  });

  it("formats minutes under an hour, floored", () => {
    expect(elapsedLabel(60_000)).toBe("1m");
    expect(elapsedLabel(119_999)).toBe("1m");
    expect(elapsedLabel(59 * 60_000)).toBe("59m");
  });

  it("formats hours, with and without a remaining minutes component", () => {
    expect(elapsedLabel(60 * 60_000)).toBe("1h");
    expect(elapsedLabel(90 * 60_000)).toBe("1h 30m");
  });

  it("clamps a negative duration to 0s rather than going negative", () => {
    expect(elapsedLabel(-500)).toBe("0s");
  });
});

describe("agentStatePresentationFor", () => {
  it("groups both waiting states under the same non-alarming amber family", () => {
    expect(agentStatePresentationFor("waiting_for_permission").className).toBe("agent-waiting");
    expect(agentStatePresentationFor("waiting_for_input").className).toBe("agent-waiting");
  });

  it("gives failed and completed their own distinct, non-pulsing classes", () => {
    const failed = agentStatePresentationFor("failed");
    expect(failed.className).toBe("agent-failed");
    expect(failed.pulse).toBe(false);
    const completed = agentStatePresentationFor("completed");
    expect(completed.className).toBe("agent-completed");
    expect(completed.pulse).toBe(false);
  });

  it("working and starting pulse as the active-work family", () => {
    expect(agentStatePresentationFor("working")).toEqual({
      label: "Working",
      className: "agent-working",
      pulse: true,
    });
    expect(agentStatePresentationFor("starting").className).toBe("agent-working");
  });
});

describe("agentRuntimeLabel", () => {
  it("maps every wire runtime token to its display label", () => {
    expect(agentRuntimeLabel("claude-code")).toBe("Claude Code");
    expect(agentRuntimeLabel("codex")).toBe("Codex");
    expect(agentRuntimeLabel("kimi")).toBe("Kimi");
    expect(agentRuntimeLabel("opencode")).toBe("OpenCode");
  });
});
