import { useEffect, useState } from "react";
import { weatherArtFor } from "../lib/weatherArt";
import { useClock } from "../useClock";
import type {
  LiveMatchSummary,
  NowPlayingSummary,
  StatusState,
  WeatherSummary,
} from "../useStatusState";

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

// plan 105 (Step B): split from the old combined `WeatherPeekScene` so the
// art (this component) can sit BEHIND the media row instead of being
// replaced by it — operator feedback wanted the weather backdrop kept once
// media took over the readout slot. `aria-hidden` because this is pure
// decoration, same as the ALERT card's own mood layer; the glyph rides
// along here (not in the readout) because it's part of the art, not the
// data — same z-index tier (0) as the mood gradient it's layered with.
//
// plan 110 (Step B): `weather.isDay` now rides the wire itself
// (status.rs's `WeatherSummary.is_day`) — the old local-hour heuristic
// (`isDaytimeNow()`, wall-clock based, wrong for coordinates outside the
// machine's timezone) is gone; this is rust's own day/night read, the
// same source the weather ALERT card's `wx-is-day` marker already used.
// Same binary serves both rust and this frontend (a Tauri app, not two
// deployables that could skew), so there is no "old rust / new frontend"
// window to guard with a clock fallback.
function WeatherPeekBackdrop({ weather }: { weather: WeatherSummary }) {
  const art = weatherArtFor(weather.condition, weather.isDay);
  const backdropClass = ["wx-peek-backdrop", "wx-card", art.moodClass, art.textureClass]
    .filter(Boolean)
    .join(" ");
  return (
    <div className={backdropClass} aria-hidden="true">
      <img className="wx-icon" src={art.glyphUrl} alt="" />
    </div>
  );
}

// plan 105 (Step B): the data half of the old `WeatherPeekScene` — markup
// unchanged from before the split, just no longer carries the art classes.
function WeatherPeekReadout({ weather }: { weather: WeatherSummary }) {
  return (
    <div className="wx-peek-readout">
      <span className="wx-peek-temp">{weather.tempDisplay}</span>
      {/* plan 092 (item 10): reuse `.chip` for the condition label —
          092 retired `.pill` entirely; a new pill here would silently
          undo that. */}
      <span className="chip wx-peek-condition">{weather.condition}</span>
    </div>
  );
}

// plan 104 (Step 7): a tiny bundle-id -> glyph map. Text glyphs only —
// artwork transport is explicitly deferred (the plan's own decision 6).
// Order matters: checked top-to-bottom, first match wins.
export function glyphForBundleId(bundleId: string | null): string {
  if (bundleId === null) {
    return "▶";
  }
  const lower = bundleId.toLowerCase();
  if (lower.includes("music")) {
    return "♪";
  }
  if (lower.includes("tv")) {
    return "📺";
  }
  if (["safari", "zen", "chrome", "firefox"].some((browser) => lower.includes(browser))) {
    return "🌐";
  }
  return "▶";
}

function formatElapsed(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

// Re-renders once a second while (and only while) playing, so the local
// elapsed-time derivation below stays live without ever driving a wire
// emission (rust never re-emits per tick — plan-081's own lesson, see
// useStatusState.ts's NowPlayingSummary doc). Honors
// prefers-reduced-motion by never arming the interval at all, so the
// component renders exactly one static value (Step 6's own requirement).
function useLiveTick(enabled: boolean) {
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!enabled) {
      return;
    }
    const reducedMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reducedMotion) {
      return;
    }
    const id = window.setInterval(() => setTick((t) => t + 1), 1000);
    return () => window.clearInterval(id);
  }, [enabled]);
}

function MediaPeekRow({ media }: { media: NowPlayingSummary }) {
  // Progress derives LOCALLY from the snapshot (plan 104 Step 6): a
  // per-tick derivation, never a per-tick wire read — `media` itself
  // only changes on a genuine adapter diff event.
  useLiveTick(media.playing);
  const liveElapsedMs = media.playing
    ? media.elapsedMs + Math.max(0, Date.now() - media.capturedAtMs)
    : media.elapsedMs;
  const clampedElapsedMs =
    media.durationMs !== null ? Math.min(liveElapsedMs, media.durationMs) : liveElapsedMs;
  const progressPct =
    media.durationMs !== null && media.durationMs > 0
      ? Math.min(100, (clampedElapsedMs / media.durationMs) * 100)
      : 0;
  const subtitle = media.artist ?? media.album ?? null;

  return (
    <div className="media-row">
      <div className="media-track">
        <span className="media-art" aria-hidden="true">
          {glyphForBundleId(media.appBundleId)}
        </span>
        <span className="media-meta">
          <span className="media-title">{media.title}</span>
          {subtitle !== null ? <span className="media-subtitle">{subtitle}</span> : null}
        </span>
      </div>
      <div className="media-transport">
        <span className="media-state" aria-hidden="true">
          {media.playing ? "▶" : "⏸"}
        </span>
        <span className="media-bar">
          <span className="media-bar-fill" style={{ width: `${progressPct}%` }} />
        </span>
        <span className="media-time">{formatElapsed(clampedElapsedMs)}</span>
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
  const media = status?.media.current ?? null;
  const weather = status?.weather.current ?? null;
  // plan 105 (Step B): the art now paints as a backdrop layer under
  // whatever the precedence chain picks, rather than being one of the
  // precedence options itself — so it survives media outranking the
  // weather readout. A live match keeps its own visual (the scorecard
  // reveal), so the weather backdrop stays out in that case.
  const showBackdrop = weather !== null && live === null;

  return (
    <div className={`below-block idle-peek${closing ? " closing" : " open"}`}>
      {showBackdrop ? <WeatherPeekBackdrop weather={weather} /> : null}
      <div className="peek-content">
        {/* plan 104: precedence is football > media > weather > timeline —
            an actively-chosen media session outranks ambient temperature,
            but a live match still outranks everything (item 3's original
            rule, extended). One below-block at a time. */}
        {live !== null ? (
          <ScorecardRevealContent live={live} />
        ) : media !== null ? (
          <MediaPeekRow media={media} />
        ) : weather !== null ? (
          <WeatherPeekReadout weather={weather} />
        ) : null}
        <PeekTimeline />
      </div>
    </div>
  );
}
