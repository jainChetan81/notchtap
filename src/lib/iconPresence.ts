// The one place `StatusState` (the ambient wire, `src/useStatusState.ts`)
// is turned into the icon strip's three `IconVisualState`s. Deliberately
// data, not logic — the same "a config table, not a new render path"
// discipline `lib/presentation.ts` follows, kept in its own file because
// it reads the STATUS wire rather than the SLOT wire that file is built
// around.
//
//   | tab      | present when                            | live when        |
//   |----------|-----------------------------------------|------------------|
//   | agent    | a session is genuinely running          | same as present  |
//   | football | a match is genuinely live               | same as present  |
//   | news     | always, whenever the strip is up        | `isCharged`      |
//
// Agent and football collapse "present" and "live" into one condition,
// which is exactly what IconStrip.tsx's own `IconVisualState` doc
// predicts ("agent/football are only ever 'hidden' or 'live' in
// practice").
import type { IconVisualState, Tab } from "../components/IconStrip";
import type { StatusState } from "../useStatusState";

export type IconPresence = Record<Tab, IconVisualState>;

/// Collapses the "present iff live" sources into one expression, so the
/// table below reads as a table rather than copies of the same ternary.
function presentAndLive(live: boolean): IconVisualState {
  return live ? "live" : "hidden";
}

/// `status` is optional for the same reason `StatusRailCard`'s own prop
/// is: the settings-window Appearance preview and most component tests
/// render that card with no status wire at all. A missing wire reads as
/// "nothing is happening", which is exactly `useStatusState`'s own
/// all-gates-off FALLBACK_STATUS — so the two always agree without this
/// file keeping a second copy of that literal.
export function iconPresenceFor(status: StatusState | undefined) {
  if (status === undefined) {
    return {
      agent: "hidden",
      football: "hidden",
      news: "present",
    } satisfies IconPresence;
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
    // always present; escalates to full weight only once the charge has
    // genuinely fired (`news.isCharged`, rust's edge-held flag —
    // `src-tauri/src/news_charge.rs`). The `chargeFraction`/`chargeCount`
    // fill and badge are a SEPARATE axis IconStrip takes as their own
    // props (`newsCharge`/`newsCount`), not part of this tier.
    news: status.news.isCharged ? "live" : "present",
  } satisfies IconPresence;
}
