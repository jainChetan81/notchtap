// Appearance gallery fixtures — extracted out of SettingsApp.tsx (plan
// 111 Step 3) so the gallery module and the settings shell can evolve
// independently. Covers the states most sensitive to the card-CSS drift
// this same plan's shared-stylesheet work (overlay-card.css) exists to
// prevent: the original four expanded samples, PLUS one compact
// (collapsed) card, one live ESPN scorecard, one weather alert, and one
// compact news card (110's single `.notif-time-inline` timestamp).
//
// Deliberately OUT (per the plan's own scoping note): idle rail / idle
// hover-peek / bare notch / reduced-motion. Those are window-level
// overlay states (idle clock, hover-driven peek reveal, the notchless-
// vs-notch shell paint) that the preview frame — a static per-sample
// `.preview-stage` box, never the real overlay window, never hover-
// driven — has no honest way to host. A dedicated dev-only living
// gallery could show them; that's out of scope here (see plan 111's
// maintenance notes).
import type { EspnMeta, SlotState } from "../useSlotState";

type ShowingSlotState = Extract<SlotState, { state: "showing" }>;

export interface PreviewSample {
  label: string;
  slot: ShowingSlotState;
}

// plan 084's structured espn meta (POST-083 contract), same base shape
// StatusRailCard.test.tsx's own ESPN_BASE fixture uses — kept in lockstep
// with that file's `EspnMeta` shape rather than inventing a new one.
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
    },
  },
  {
    label: "Generic alert (High priority, cmux)",
    slot: {
      state: "showing",
      id: "preview-cmux",
      title: "Agent needs input",
      body: "run `git push origin master`?",
      eventType: "generic",
      priority: "high",
      signal: "generic",
      // this sample is the cmux accent's own preview vehicle (plan 096) —
      // it's the one origin the settings preview can actually show the
      // accent for; label above says "cmux" for exactly this reason.
      origin: "cmux",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      // preview samples keep subtitle/details empty (plan 035): the render
      // path for populated cells is exercised in StatusRailCard.test.tsx.
      subtitle: null,
      details: [],
      queueTotal: 3,
      queueDone: 2,
      ttlMs: 8000,
      remainingMs: 5000,
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
      category: "politics",
      publishedAtMs: null,
      link: "https://example.com/digital-rights",
      subtitle: null,
      details: [],
      queueTotal: 1,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 5000,
    },
  },
  // plan 111 (Step 3): the states most sensitive to CSS drift are exactly
  // the ones the old four-expanded-sample gallery could never show —
  // added below, one per state.
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
      espn: ESPN_BASE,
    },
  },
  {
    label: "Weather alert (medium priority)",
    slot: {
      state: "showing",
      id: "preview-weather",
      title: "Rain expected soon",
      body: "75% chance of rain within ~30 min",
      eventType: "generic",
      priority: "medium",
      signal: "generic",
      origin: "weather",
      expanded: false,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      subtitle: null,
      // plan 082's wx-condition/wx-is-day marker pair (plan 035's details
      // channel, reused) — same shape StatusRailCard.test.tsx's own
      // WEATHER_ALERT fixture uses. plan 110 reads wx-is-day for the
      // wire-driven day/night art variant; "1" is day.
      details: [
        { label: "wx-condition", value: "Rain" },
        { label: "wx-is-day", value: "1" },
      ],
      queueTotal: 1,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 5000,
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
      // plan 110: news collapses to ONE timestamp — a non-null
      // publishedAtMs here exercises that single-stamp compact render
      // (age reads from `.notif-time-inline`, not a duplicated pill).
      // Fixed epoch ms (not Date.now()) so the gallery renders
      // deterministically — required for the frozen-state rendered-
      // equivalence screenshots (plan 111 Step 2).
      publishedAtMs: 2_000_000_000_000 - 5 * 60_000,
      link: "https://example.com/markets",
      subtitle: null,
      details: [],
      queueTotal: 1,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 5000,
    },
  },
];
