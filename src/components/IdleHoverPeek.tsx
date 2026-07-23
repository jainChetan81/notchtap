import { Globe, type LucideIcon, Music, Pause, Play, Tv } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { NOTCHTAP_EASE, ROTATION_ENTER_MS } from "../animationTiming";
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
// Mount lifecycle: the CONTENT below opens/closes on the `open` prop via
// `AnimatePresence` + `motion.div` (plan 12x — migrated off a hand-rolled
// mounted/closing `useState` + `setTimeout` state machine and matching CSS
// `@keyframes`, this codebase's animation-law now routes all animation
// through the `motion` library rather than hand-drawn keyframes or manual
// layout-prop tweening). `AnimatePresence` owns the exit window itself
// (`exit={...}` plays out before the node actually leaves the DOM) — no
// purpose-built timer needed here anymore. `IdleHoverPeek` ITSELF is now
// always mounted by StatusRailCard (plan 127, Step 2, finding #2) — only
// `open` (defaulting to `hovered` for standalone callers, see the prop's
// own doc below) gates this AnimatePresence child, so a promotion
// arriving mid-peek (which flips `open` false without ever unmounting
// this component) lets the exit actually play instead of the whole
// subtree — AnimatePresence included — being torn out synchronously by a
// parent-level conditional.
//
// This is also why the block is NOT always-mounted whenever ambient data
// exists: `.below-block`'s mere DOM presence is what 091's
// `:not(:has(.below-block))` rounding law keys off (untouched by this
// plan — the flanks-un-round-while-open behavior below falls out of that
// existing rule for free, exactly because this mounts only while open,
// including during `AnimatePresence`'s exit animation). Always-mounting
// whenever weather/football happened to be configured would un-round the
// idle pill any time that ambient data exists, not just while actually
// hovered — a real, unwanted change to 091's shell behavior for the
// common case of "weather enabled, not currently hovering."

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
//
// plan 122: this backdrop is now PURE art (mood gradient + texture) — the
// condition glyph moved out to `WeatherPeekReadout` below. It used to be
// absolutely positioned in this block's top-right corner
// (`overlay-card.css`'s old `.wx-icon` rule), the SAME corner the media
// row's transport (play/pause + bar + elapsed time) is flex-pinned to —
// whenever media played, the two occupied the same region. Moving the
// glyph into the readout's in-flow row makes that collision impossible
// by construction: the readout (and its glyph) simply doesn't render at
// all while media/football own the content slot below, so there is
// nothing left to overlap the transport with.
function WeatherPeekBackdrop({ weather }: { weather: WeatherSummary }) {
  const art = weatherArtFor(weather.condition, weather.isDay);
  const backdropClass = ["wx-peek-backdrop", "wx-card", art.moodClass, art.textureClass]
    .filter(Boolean)
    .join(" ");
  return <div className={backdropClass} aria-hidden="true" />;
}

// plan 105 (Step B): the data half of the old `WeatherPeekScene`.
//
// plan 122: the condition glyph now lives here too, in flow as a flex
// sibling of `.wx-peek-temp`/`.wx-peek-condition` — `.wx-peek-icon` is a
// class DEDICATED to this context, deliberately not the `.wx-icon` the
// weather ALERT card (`StatusRailCard.tsx`) still uses: that card's own
// glyph stays absolutely positioned in its own corner (untouched,
// non-goal), and the two usages share nothing beyond "a weather glyph
// image" — reusing the class would have meant either breaking the alert
// card's layout or fighting its absolute-positioning rule with an
// override, so this gets its own name and its own in-flow rule instead
// (`overlay-card.css`). Sized smaller (28px) than the alert card's 48px
// so the row doesn't grow taller than the readout's previous height —
// this peek's outer block is a fixed 100px (`hover.rs`'s
// `IDLE_PEEK_BELOW_BLOCK_H` mirror), not intrinsic to content.
//
// The rain-chance chip is also new (plan 122): `weather.rainPct` already
// arrives floor-filtered against the operator's configured
// `weather_rain_threshold_pct` (status.rs's `WeatherSummary.rain_pct` doc
// comment) — this window has no config access (receive-only, no
// invoke), so the only job here is a presence check, never re-deriving
// or comparing against the floor itself.
function WeatherPeekReadout({ weather }: { weather: WeatherSummary }) {
  const art = weatherArtFor(weather.condition, weather.isDay);
  return (
    <div className="wx-peek-readout">
      <img className="wx-peek-icon" src={art.glyphUrl} alt="" />
      <span className="wx-peek-temp">{weather.tempDisplay}</span>
      {/* plan 092 (item 10): reuse `.chip` for the condition label —
          092 retired `.pill` entirely; a new pill here would silently
          undo that. */}
      <span className="chip wx-peek-condition">{weather.condition}</span>
      {/* plan 127 (Step 7, missed opportunity): `rainPct` can flip
          null<->number mid-peek (a live status update, not just on
          mount/unmount of the peek itself) — the chip used to
          mount/unmount bare on that flip. `AnimatePresence` with
          `initial={false}` (a SIBLING instance, not the outer peek
          container's own — this window opening/closing is a separate
          gesture from the chip appearing/disappearing within an
          already-open peek): `initial={false}` means the chip renders at
          full opacity immediately if it's already present the very
          render the peek itself opens (no double animation stacking the
          chip's own fade on top of the container's spring), while a
          LATER flip (peek already open, rainPct newly arrives or
          clears) still fades. Duration reuses ROTATION_ENTER_MS
          (animationTiming.ts) — not a new token: a fast, light UI-chip
          fade calls for the same ~120ms feel Step 3's rotation enter
          already established, and this codebase's animation-law
          (desynced clocks are the bug class, not "one token per call
          site") is satisfied by sharing an existing single-sourced
          duration rather than minting a near-duplicate one for a single
          low-traffic consumer. */}
      <AnimatePresence initial={false}>
        {weather.rainPct !== null ? (
          <motion.span
            key="wx-peek-rain"
            className="chip wx-peek-rain"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: ROTATION_ENTER_MS / 1000, ease: NOTCHTAP_EASE }}
          >
            Rain {weather.rainPct}%
          </motion.span>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

// plan 104 (Step 7): a tiny bundle-id -> icon map. plan 118 swapped the
// text-glyph transport for lucide components (rendered, not measured as
// text) — order and branch conditions are unchanged from the original,
// only the return type moved from an emoji string to the icon component
// itself. Order matters: checked top-to-bottom, first match wins.
export function iconForBundleId(bundleId: string | null): LucideIcon {
  if (bundleId === null) {
    return Play;
  }
  const lower = bundleId.toLowerCase();
  if (lower.includes("music")) {
    return Music;
  }
  if (lower.includes("tv")) {
    return Tv;
  }
  if (["safari", "zen", "chrome", "firefox"].some((browser) => lower.includes(browser))) {
    return Globe;
  }
  return Play;
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
// useStatusState.ts's NowPlayingSummary doc).
function useLiveTick(enabled: boolean) {
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!enabled) {
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
  const MediaIcon = iconForBundleId(media.appBundleId);

  return (
    <div className="media-row">
      <div className="media-track">
        <span className="media-art" aria-hidden="true">
          <MediaIcon className="media-art-icon" aria-hidden="true" />
        </span>
        <span className="media-meta">
          <span className="media-title">{media.title}</span>
          {subtitle !== null ? <span className="media-subtitle">{subtitle}</span> : null}
        </span>
      </div>
      <div className="media-transport">
        <span className="media-state" aria-hidden="true">
          {media.playing ? (
            <Play className="media-state-icon" aria-hidden="true" />
          ) : (
            <Pause className="media-state-icon" aria-hidden="true" />
          )}
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

export function IdleHoverPeek({
  status,
  hovered,
  // plan 127 (Step 2, /improve-animations audit finding #2): the mount
  // gate used to be StatusRailCard's own `{!renderedShowing && <IdleHoverPeek
  // hovered={hovered} />}` conditional — which unmounted this WHOLE
  // component (AnimatePresence included) the instant a promotion arrived
  // mid-peek, tearing out up to 100px of content with zero animation
  // (React drops a component tree synchronously; an AnimatePresence
  // inside the removed subtree never gets the chance to run its own
  // `exit`). `open` is the new, dedicated mount-gate signal — sourced by
  // the caller from `!renderedShowing && hovered` exactly as before, but
  // now read INSIDE the always-mounted component's own AnimatePresence
  // condition, so a promotion flipping `open` false lets THIS
  // AnimatePresence play the existing `exit={...}` collapse for real
  // while the caller's card content enters above it, instead of
  // vanishing instantly. Defaults to `hovered` (this component's
  // original, pre-127 gate) so every standalone caller — this file's own
  // tests, the settings preview, any future direct usage — keeps
  // identical behavior without having to pass a redundant `open` prop
  // when there's no separate "peek should survive a promotion" caller to
  // coordinate with.
  open = hovered,
}: {
  status?: StatusState;
  hovered: boolean;
  open?: boolean;
}) {
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
    <AnimatePresence>
      {open ? (
        // plan 093: `height: 100` mirrors `hover.rs`'s `IDLE_PEEK_BELOW_BLOCK_H`
        // constant exactly — a real duplicated-constants pair (see that
        // constant's own doc comment). Any change to this height MUST
        // change that constant in the same commit.
        <motion.div
          className="below-block idle-peek"
          /* 2026-07-23 review fix (the "peek close pops ~25px" finding):
             `.idle-peek`'s CSS `padding: 12px 16px 13px` + border-box means
             a bare `height: 0` FLOORS at padding-top+bottom (25px) — the
             box could never reach zero, so the close visibly popped at
             unmount. Animating the vertical paddings in lockstep with the
             height (12/13 at rest, matching the stylesheet exactly, so
             there is zero visual change while open) lets the collapse
             genuinely reach 0. Horizontal padding stays CSS-owned (16px). */
          initial={{ height: 0, opacity: 0, paddingTop: 0, paddingBottom: 0 }}
          animate={{ height: 100, opacity: 1, paddingTop: 12, paddingBottom: 13 }}
          exit={{ height: 0, opacity: 0, paddingTop: 0, paddingBottom: 0 }}
          // plan 12x (wave 3): stiffer spring (420 -> 480) and a quicker
          // opacity fade (180ms -> 150ms), operator-feedback "snappier
          // overall" pass — damping nudged up in step (34 -> 37) to keep
          // the same near-critically-damped feel (damping/stiffness ratio
          // ~0.081 before, ~0.077 now) rather than trading the speed gain
          // for extra bounce/overshoot. `height: 100` is untouched (rust's
          // `IDLE_PEEK_BELOW_BLOCK_H` pairing, out of scope for this pass).
          transition={{ type: "spring", stiffness: 480, damping: 37, opacity: { duration: 0.15 } }}
          style={{ overflow: "hidden" }}
        >
          {showBackdrop ? <WeatherPeekBackdrop weather={weather} /> : null}
          <div className="peek-content">
            {/* plan 104: precedence is football > media > weather >
                timeline — an actively-chosen media session outranks
                ambient temperature, but a live match still outranks
                everything (item 3's original rule, extended). One
                below-block at a time. */}
            {live !== null ? (
              <ScorecardRevealContent live={live} />
            ) : media !== null ? (
              <MediaPeekRow media={media} />
            ) : weather !== null ? (
              <WeatherPeekReadout weather={weather} />
            ) : null}
            <PeekTimeline />
          </div>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}
