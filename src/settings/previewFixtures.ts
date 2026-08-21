// Appearance gallery fixtures — kept separate from the settings shell so
// the two evolve independently. Covers the states most sensitive to
// card-CSS drift: the four expanded samples PLUS one compact (collapsed)
// card, one live ESPN scorecard, and one compact news card (single
// `.notif-time-inline` timestamp).
//
// Deliberately OUT: idle rail / idle hover-peek / bare notch — window-level
// overlay states (idle clock, hover-driven peek, notchless-vs-notch shell
// paint) a static per-sample `.preview-stage` box has no honest way to host.
import type { EspnMeta, SlotState } from "../hooks/useSlotState";

type ShowingSlotState = Extract<SlotState, { state: "showing" }>;

export interface PreviewSample {
  label: string;
  slot: ShowingSlotState;
}

// Same base shape as StatusRailCard.test.tsx's ESPN_BASE fixture — kept
// in lockstep with that file's `EspnMeta` shape.
const ESPN_BASE: EspnMeta = {
  league: "UCL",
  homeAbbrev: "ARS",
  awayAbbrev: "PSG",
  homeScore: 1,
  awayScore: 1,
  clock: "78'",
  homeCards: [0, 0],
  awayCards: [0, 0],
  homeCrest: null,
  awayCrest: null,
};

export const PREVIEW_SAMPLES: ReadonlyArray<PreviewSample> = [
  {
    label: "Goal (High priority, football)",
    slot: {
      state: "showing",
      id: "preview-goal",
      title: "GOAL",
      body: "Arsenal 2-0",
      eventType: "score_update",
      priority: "high",
      signal: "goal",
      origin: "football",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      subtitle: null,
      details: [],
      queueTotal: 3,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 5000,
      agentRuntime: null,
    },
  },
  {
    label: "Red card (High priority, football)",
    slot: {
      state: "showing",
      id: "preview-red-card",
      title: "Red Card",
      body: "Chelsea down to 10",
      eventType: "match_state",
      priority: "high",
      signal: "red_card",
      origin: "football",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      subtitle: null,
      details: [],
      queueTotal: 3,
      queueDone: 1,
      ttlMs: 8000,
      remainingMs: 5000,
      agentRuntime: null,
    },
  },
  {
    label: "Generic alert (High priority, agent)",
    slot: {
      state: "showing",
      id: "preview-agent",
      title: "Agent needs input",
      body: "run `git push origin master`?",
      eventType: "generic",
      priority: "high",
      signal: "generic",
      // The one origin the settings preview can show the agent accent
      // for — label above says "agent" for this reason.
      origin: "agent",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      // Sample cells keep subtitle/details empty — populated-cell render
      // path is exercised in StatusRailCard.test.tsx.
      subtitle: null,
      details: [],
      queueTotal: 3,
      queueDone: 2,
      ttlMs: 8000,
      remainingMs: 5000,
      agentRuntime: "claude-code",
    },
  },
  {
    label: "News headline (Low priority)",
    slot: {
      state: "showing",
      id: "preview-news",
      title: "Parliament passes the landmark digital rights bill",
      body: "The measure passed after a late-night vote.",
      eventType: "news_item",
      priority: "low",
      signal: "generic",
      origin: "news",
      expanded: true,
      source: "NDTV",
      category: "science",
      publishedAtMs: null,
      link: "https://example.com/digital-rights",
      subtitle: null,
      details: [],
      queueTotal: 1,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 5000,
      agentRuntime: null,
    },
  },
  // The states most sensitive to CSS drift, one per sample below:
  // compact (collapsed), recurring live scorecard, single-stamp news.
  {
    label: "Compact (collapsed manifest, medium priority)",
    slot: {
      state: "showing",
      id: "preview-compact",
      title: "Build finished",
      body: "notchtap run completed in 42s",
      eventType: "generic",
      priority: "medium",
      signal: "generic",
      origin: "manual",
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
      remainingMs: 5000,
      agentRuntime: null,
    },
  },
  {
    label: "Live match (recurring scorecard, football)",
    slot: {
      state: "showing",
      id: "preview-live-match",
      title: "UCL: ARS 1-1 PSG",
      body: "Goal — K. Havertz 78'",
      eventType: "score_update",
      priority: "high",
      signal: "goal",
      origin: "football",
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
      remainingMs: 5000,
      agentRuntime: null,
      espn: ESPN_BASE,
    },
  },
  {
    label: "News headline, compact (single timestamp)",
    slot: {
      state: "showing",
      id: "preview-news-compact",
      title: "Markets close mixed after late rally",
      body: "Trading was volatile through the afternoon session.",
      eventType: "news_item",
      priority: "low",
      signal: "generic",
      origin: "news",
      expanded: false,
      source: "NDTV",
      category: "business",
      // News collapses to ONE timestamp — non-null publishedAtMs exercises
      // the single-stamp compact render (age reads from
      // `.notif-time-inline`, not a duplicated pill). Fixed epoch ms (not
      // Date.now()) so the gallery renders deterministically.
      publishedAtMs: 2_000_000_000_000 - 5 * 60_000,
      link: "https://example.com/markets",
      subtitle: null,
      details: [],
      queueTotal: 1,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 5000,
      agentRuntime: null,
    },
  },
];
