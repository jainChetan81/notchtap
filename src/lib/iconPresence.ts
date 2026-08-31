import type { IconVisualState, Tab } from "../components/IconStrip";
import type { StatusState } from "../hooks/useStatusState";

export type IconPresence = Record<Tab, IconVisualState>;

function presentAndLive(live: boolean): IconVisualState {
  return live ? "live" : "hidden";
}

export function iconPresenceFor(status: StatusState | undefined) {
  if (status === undefined) {
    return {
      agent: "hidden",
      football: "hidden",
      news: "present",
    } satisfies IconPresence;
  }
  return {
    agent: presentAndLive(status.agent.activeSessions > 0),
    football: presentAndLive(status.football.live !== null),
    news: status.news.isCharged ? "live" : "present",
  } satisfies IconPresence;
}
