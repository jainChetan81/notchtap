import { useEffect, useState } from "react";
import { weatherArtFor } from "../lib/weatherArt";
import { useClock } from "../useClock";
import type { LiveMatchSummary, StatusState, WeatherSummary } from "../useStatusState";

// plan 093 (079 items 9/17/18, folded into one surface): the idle
// hover-expanded state — hovering the idle assembly (`hovered`, the hover
// primitive's prop, NEVER CSS `:hover` — the overlay window is
// click-through, only the rust tracking area knows the cursor) opens a
// `.below-block` beneath the flank row. Content precedence (item 3's
// design constraint — football outranks ambient weather, one below-block
// at a time): a live match (`status.football.live`) wins over the weather
// scene (`status.weather.current`); with neither available, the block
// still opens with the day-progress timeline alone (item 18's decision:
// "the timeline appears only in the idle expanded-on-hover view", read as
// unconditional on ambient data — the timeline lost every other home when
// 091 removed the old always-on idle view, so gating its one remaining
// home on weather/football being configured would make it permanently
// unreachable for anyone without either).
//
// Mount lifecycle: this component mounts (and its content animates in via
// CSS `@keyframes`, this codebase's own established mount-transition
// idiom — `.card-content`'s enter/exit animations, styles.css) only while
// `hovered` is true, staying mounted for `CLOSE_DELAY_MS` after `hovered`
// goes false so the close keyframe can play, then unmounting — same shape
// as `useDelayedSwap`'s exiting window elsewhere in this file tree, but
// NOT that hook itself: `useDelayedSwap` freezes an OLD value across a
// KEY change (built for "show what was there while it fades"), whereas
// this needs "mount instantly on open, delay only the unmount on close" —
// a materially different shape, so this is a small purpose-built effect
// rather than a mismatched reuse.
//
// This is also why the block is NOT always-mounted whenever ambient data
// exists: `.below-block`'s mere DOM presence is what 091's
// `:not(:has(.below-block))` rounding law keys off (untouched by this
// plan — the flanks-un-round-while-open behavior below falls out of that
// existing rule for free, exactly because this mounts only while open).
// Always-mounting whenever weather/football happened to be configured
// would un-round the idle pill any time that ambient data exists, not
// just while actually hovered — a real, unwanted change to 091's shell
// behavior for the common case of "weather enabled, not currently
// hovering."
const CLOSE_DELAY_MS = 260;

// plan 093: `weatherArtFor` (082, reused per this plan's own citation)
// needs a day/night flag the ambient `WeatherSummary` wire shape doesn't
// carry (`status.rs`'s `WeatherSummary` is `{ tempDisplay, condition }`
// only — unlike the weather ALERT card's `wx-is-day` detail marker, there
// is no real sunrise/sunset value on this channel, and extending the wire
// shape is out of this plan's scope, `src-tauri/src/status.rs` not being
// among the in-scope files). A plain local-hour heuristic stands in
// instead — less precise than the ALERT card's rust-computed flag, but a
// reasonable approximation for a purely decorative mood scene.
function isDaytimeNow(): boolean {
  const hour = new Date().getHours();
  return hour >= 6 && hour < 18;
}

function WeatherPeekScene({ weather }: { weather: WeatherSummary }) {
  const art = weatherArtFor(weather.condition, isDaytimeNow());
  const sceneClass = ["wx-peek-scene", "wx-card", art.moodClass, art.textureClass]
    .filter(Boolean)
    .join(" ");
  return (
    <div className={sceneClass}>
      <img className="wx-icon" src={art.glyphUrl} alt="" />
      <div className="wx-peek-readout">
        <span className="wx-peek-temp">{weather.tempDisplay}</span>
        {/* plan 092 (item 10): reuse `.chip` for the condition label —
            092 retired `.pill` entirely; a new pill here would silently
            undo that. */}
        <span className="chip wx-peek-condition">{weather.condition}</span>
      </div>
    </div>
  );
}

function ScorecardRevealContent({ live }: { live: LiveMatchSummary }) {
  return (
    <div className="idle-reveal-scorecard">
      <div className="sc-head">
        <span className="chip chip-live">
          <span className="live-dot" aria-hidden="true" />
          Live
        </span>
        <span className="clock-pill">{live.minute}</span>
      </div>
      <div className="idle-reveal-label">{live.label}</div>
    </div>
  );
}

// plan 093 (item 18): the day-progress timeline, relocated from the
// deleted `IdleView`/`.idle-view .timeline` (091 removed the old always-on
// idle view; the original CSS is gone too, so this rebuilds the same
// thin-line-plus-dot shape from that history rather than reusing dead
// selectors) into its new, sole home: this peek. `useClock`'s
// `dayProgress` is unchanged — same 0–100 "how far through the local day"
// value the old view read.
function PeekTimeline() {
  const { dayProgress } = useClock();
  return (
    <span
      className="idle-peek-timeline"
      style={{ "--day-progress": `${dayProgress}%` } as React.CSSProperties}
      aria-hidden="true"
    />
  );
}

export function IdleHoverPeek({ status, hovered }: { status?: StatusState; hovered: boolean }) {
  const [mounted, setMounted] = useState(hovered);
  const [closing, setClosing] = useState(false);

  useEffect(() => {
    if (hovered) {
      setMounted(true);
      setClosing(false);
      return;
    }
    if (!mounted) {
      return;
    }
    const reducedMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reducedMotion) {
      setMounted(false);
      setClosing(false);
      return;
    }
    setClosing(true);
    const id = window.setTimeout(() => {
      setMounted(false);
      setClosing(false);
    }, CLOSE_DELAY_MS);
    return () => window.clearTimeout(id);
  }, [hovered, mounted]);

  if (!mounted) {
    return null;
  }

  const live = status?.football.live ?? null;
  const weather = status?.weather.current ?? null;

  return (
    <div className={`below-block idle-peek${closing ? " closing" : " open"}`}>
      {/* football outranks ambient weather (item 3's precedence rule) —
          one below-block at a time. */}
      {live !== null ? (
        <ScorecardRevealContent live={live} />
      ) : weather !== null ? (
        <WeatherPeekScene weather={weather} />
      ) : null}
      <PeekTimeline />
    </div>
  );
}
