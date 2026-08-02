// Plan 171 (tab-notch redesign, slice K): the one place `StatusState`
// (the ambient wire, `src/useStatusState.ts`) is turned into the icon
// strip's five `IconVisualState`s. Deliberately data, not logic — the
// same "a config table, not a new render path" discipline
// `lib/presentation.ts` follows, kept in its own file because it reads
// the STATUS wire (`useStatusState.ts`) rather than the SLOT wire
// (`useSlotState.ts`) that file is otherwise built around.
//
// Spec `docs/superpowers/specs/2026-08-02-tab-notch-design.md` section
// 6's table, transcribed literally:
//
//   | tab      | present when                            | live when        |
//   |----------|-----------------------------------------|------------------|
//   | agent    | a session is genuinely running          | same as present  |
//   | football | a match is genuinely live               | same as present  |
//   | music    | audio is genuinely playing (see below)  | `playing`        |
//   | weather  | always, whenever the strip is up        | never            |
//   | news     | always, whenever the strip is up        | `isCharged`      |
//
// Three of the five collapse "present" and "live" into one condition,
// which is exactly what IconStrip.tsx's own `IconVisualState` doc
// predicts ("agent/football/music are only ever 'hidden' or 'live' in
// practice"). Music is the one genuine exception this mapping resolves:
// `media.current` can be non-null while PAUSED — a real, common state
// the wire distinguishes (`NowPlayingSummary.playing`) — so music is
// present-but-dim while paused and full weight only while audio is
// genuinely moving. That is a strictly finer reading of section 6's
// "present when: audio is genuinely playing" than collapsing it would
// be, and it is what makes the strip not jump when a track is paused
// mid-hover (spec section 2 decision 4: presence is a width collapse,
// and a paused track should not trigger one).
//
// Weather's "live: never" is not an oversight — section 6's own table
// leaves its live column empty, and `icon-strip.css` animates weather on
// `.is-present` rather than `.is-live` precisely because of it.
import type { IconVisualState, Tab } from "../components/IconStrip";
import type { StatusState } from "../useStatusState";

export type IconPresence = Record<Tab, IconVisualState>;

/// Collapses the three "present iff live" sources into one expression, so
/// the table below reads as a table rather than as three copies of the
/// same ternary.
function presentAndLive(live: boolean): IconVisualState {
  return live ? "live" : "hidden";
}

/// `status` is optional for the same reason `StatusRailCard`'s own prop
/// is: the settings-window Appearance preview and most component tests
/// render that card with no status wire at all. A missing wire reads as
/// "nothing is happening", which is exactly `useStatusState`'s own
/// all-gates-off FALLBACK_STATUS — so the two always agree without this
/// file keeping a second copy of that literal.
export function iconPresenceFor(status: StatusState | undefined): IconPresence {
  if (status === undefined) {
    return {
      agent: "hidden",
      football: "hidden",
      music: "hidden",
      weather: "present",
      news: "present",
    };
  }
  return {
    // present iff at least one Agent Session is registered; for agent,
    // present IS live (a registered session is by definition a running
    // one — `StatusState.agent.activeSessions`'s own wire doc).
    agent: presentAndLive(status.agent.activeSessions > 0),
    // `football.live` is a single `Option` on the wire (a `LiveMatchSummary`
    // or null) — its mere presence already means "in play", so there is
    // no second liveness field to read.
    football: presentAndLive(status.football.live !== null),
    // the one two-tier source among the first three — see the header note.
    music:
      status.media.current === null ? "hidden" : status.media.current.playing ? "live" : "present",
    // the always-present ambient icon: never escalates, never collapses.
    weather: "present",
    // always present; escalates to full weight only once the charge has
    // genuinely fired (`news.isCharged`, rust's edge-held flag —
    // `src-tauri/src/news_charge.rs`). The `chargeFraction`/`chargeCount`
    // fill and badge are a SEPARATE axis IconStrip takes as their own
    // props (`newsCharge`/`newsCount`), not part of this tier.
    news: status.news.isCharged ? "live" : "present",
  };
}
