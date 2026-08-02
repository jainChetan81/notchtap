// Plan 171 (tab-notch redesign, slice H): the media tab's below-block —
// mounted by whichever parent owns the icon-strip's hover-with-a-
// selection shell (slice K's integration; this component only renders
// what goes INSIDE `.below-block`, the same scope every other slice in
// this plan keeps to). Spec section 7's media bullet, verbatim:
//
//   "poured into the same skeleton (media kicker, track title as
//   .title.headline, artist as the subtitle row), then the album tile +
//   transport buttons + the shipped .media-bar ... in the body area.
//   Transport buttons (prev/play-pause/next) are new ... No TTL anywhere
//   on this page — media is a live surface, not an event."
//
// Design reference: prototypes/tab-notch-panel.html, data-page="music"
// (~line 1003-1039) — the ONLY place `.media-body`/`.mr-btn`/`.mr-glyph`/
// `.media-scrub`/`.media-times`/`.queue-list` markup exists today; none of
// it is shipped CSS yet (confirmed via repo-wide grep before writing
// this), so this slice adds a small new stylesheet
// (`src/overlay/media-below-block.css`) for exactly those classes,
// following the same "new surface gets its own file, appended to
// overlay-card.css's import list" convention icon-strip.css/eq-bars.css
// (slice E) already established this same session — `.media-bar`/
// `.media-bar-fill` themselves are NOT re-declared there; they're reused
// verbatim from the shipped `idle-peek.css` rule (see the progress-bar
// comment below).
//
// No Stamp badge: the mock's own `<div class="stamp">Live</div>` is a
// hand-typed placeholder, not a real `<Stamp priority=... signal=...
// eventType=... />` call — `NowPlayingSummary` (useStatusState.ts)
// carries no `Priority`/`EventSignal`/`EventType` at all (media is an
// ambient PULL surface, never a push event; now_playing.rs's own doc:
// "media never becomes an Event/card"). Inventing values to satisfy
// Stamp's props would be exactly the kind of improvised, undocumented
// data this plan's "flagged, not improvised" discipline (see slice G's
// `secondaryMatches` gap note) warns against, and the task's own item
// list for this component's compact card never asked for one — so it's
// simply omitted here, not silently faked.
//
// No album-art image: `NowPlayingSummary` carries no artwork field —
// this is the SAME already-established decision idle-peek.css's own
// `.media-row` comment documents ("Text glyphs only (no artwork
// transport, per the plan's own decision 6)"), not a new gap this
// component introduces. `.media-art.big` renders the same plain note
// glyph the mock itself uses.
import { useEffect, useRef } from "react";
import type { NowPlayingSummary } from "../useStatusState";

// Slice C's own wire shape (`src-tauri/src/now_playing.rs`'s
// `MediaCommand`), mirrored here as the three lower-camel string
// literals this callback fires — NOT an invoke() call and NOT a new
// `#[tauri::command]` (CLAUDE.md's ipc/security section; spec section
// 10's explicit "the overlay stays receive-only for commands" rule).
// Real click detection is Slice A's own still-open Mac-hardware
// question (plan 171 §Slice A item 2) and command dispatch is Slice C's
// (`send_command`, not yet wired to any caller) — this component only
// ever renders the buttons and reports which one fired; a future
// integration layer decides what happens next.
export type MediaCommand = "previous" | "playPause" | "next";

// Mirrors IdleHoverPeek.tsx's own `formatElapsed` (component-local,
// unexported there too) rather than importing it — this component must
// not touch IdleHoverPeek.tsx (a different slice's file; plan 171's own
// per-slice file boundaries), and the function is a single, trivial,
// side-effect-free formatter with nothing to drift.
function formatElapsed(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

// Deliberately simpler than IdleHoverPeek.tsx's own `MediaPeekRow`: that
// component runs a `useLiveTick` 1s re-render loop and derives elapsed as
// `elapsedMs + (Date.now() - capturedAtMs)` while playing, so a
// continuously-mounted ambient row stays accurate between wire diffs.
// This below-block isn't wired into any live mount point yet (slice K's
// job — see the file-header comment above and this plan's own "not
// integrated" boundary), so it renders progress directly off the
// snapshot's `elapsedMs`/`durationMs` — accurate as of whatever
// `NowPlayingSummary` the caller last passed in, recomputed on every
// render exactly like the source of truth being read literally, no
// `Date.now()` anywhere in this file. If the pulled below-block turns
// out to need the live per-second creep once it's actually mounted
// continuously (slice K), that's a one-line addition (borrow
// `useLiveTick` verbatim) at that point, not a redesign — the discontinuity
// guard below (the part of IdleHoverPeek's pattern the task brief
// explicitly points at, lines 300-358) is reused faithfully regardless,
// since it depends only on prop changes between renders, not on a timer.
function progressFor(media: NowPlayingSummary): { progressPct: number; clampedElapsedMs: number } {
  const clampedElapsedMs =
    media.durationMs !== null ? Math.min(media.elapsedMs, media.durationMs) : media.elapsedMs;
  const progressPct =
    media.durationMs !== null && media.durationMs > 0
      ? Math.min(100, (clampedElapsedMs / media.durationMs) * 100)
      : 0;
  return { progressPct, clampedElapsedMs };
}

// Same "artist · album, or just the one that's present" composition
// AgentBoard.tsx's `agentHeroPropsFor` uses for its own subtitle
// (`` `${runtimeLabel} · ${projectName}` : runtimeLabel ``) — extended
// here because BOTH fields are nullable on `NowPlayingSummary` (unlike
// `runtimeLabel`, which is never null), so there's a third case that
// function's shape doesn't need: neither present, no subtitle row at
// all (same "nothing to show" posture the rest of this plan's
// below-blocks use for missing data, just scoped to one row instead of
// the whole card).
function mediaSubtitle(media: NowPlayingSummary): string | null {
  if (media.artist !== null && media.album !== null) {
    return `${media.artist} · ${media.album}`;
  }
  return media.artist ?? media.album;
}

export function MediaBelowBlock({
  media,
  expanded = false,
  queue = [],
  onCommand,
}: {
  media: NowPlayingSummary | null;
  /// Gates the scrubber (`.media-scrub`/`.media-times`) and queue preview
  /// (`.queue-list`) per spec section 7/9 — `prefix+enter` is what flips
  /// this in the real app; this component only renders what's asked.
  expanded?: boolean;
  /// No wire concept of a queue exists today (grepped `NowPlayingSummary`
  /// and the rest of `useStatusState.ts` before writing this — confirmed
  /// absent). Following slice G's own precedent for exactly this
  /// situation (`FootballHeroCard`'s `secondaryMatches`: a real,
  /// documented, optional prop with no real caller yet, flagged rather
  /// than improvised) instead of inventing placeholder data: an omitted
  /// or empty `queue` renders no `.queue-list` at all, even when
  /// `expanded` is true. A future plan step wiring a real queue source
  /// needs no change to this component's shape, only a real value.
  queue?: { title: string; artist: string }[];
  /// Presentational only — see the `MediaCommand` doc above. Never
  /// invoked by this component itself; only ever forwarded from a
  /// button's onClick.
  onCommand?: (command: MediaCommand) => void;
}) {
  // Same "nothing to show, render nothing" posture AgentBelowBlock.tsx's
  // `sessions.length === 0` guard and PositionBar.tsx's `total <= 0`
  // guard use — no now-playing session means there is nothing this
  // below-block can meaningfully draw.
  //
  // Hooks below must still run unconditionally on every render (rules of
  // hooks), so the guard reads `media` directly rather than an early
  // return before the hooks — see the `media === null` fallback threaded
  // through the discontinuity effect below.
  const prevRef = useRef<{ progressPct: number; title: string } | null>(null);
  const { progressPct, clampedElapsedMs } =
    media !== null ? progressFor(media) : { progressPct: 0, clampedElapsedMs: 0 };

  // Faithful port of IdleHoverPeek.tsx's own discontinuity guard
  // (:300-358, the exact pattern the task brief points at): CSS's
  // `transition: transform 1s linear` on `.media-bar-fill` (idle-peek.css
  // — reused verbatim, not re-declared by this slice's new stylesheet)
  // exists for a steady playback glide; suppressing it inline on any
  // discontinuity (first render, a paused transport, progress going
  // backwards, or a title change) keeps a genuine reset reading as a
  // reset instead of a second-long slide. `prev` is written in an effect
  // (never during render) so a re-render for any other reason can't
  // mistake itself for a tick.
  const prev = prevRef.current;
  const discontinuity =
    media === null ||
    !media.playing ||
    prev === null ||
    prev.title !== media.title ||
    progressPct < prev.progressPct;
  useEffect(() => {
    if (media !== null) {
      prevRef.current = { progressPct, title: media.title };
    }
  });

  if (media === null) {
    return null;
  }

  const subtitle = mediaSubtitle(media);
  const showExpandedDetail = expanded;
  const durationDisplay = media.durationMs !== null ? formatElapsed(media.durationMs) : "--:--";

  return (
    <div className="below-block" data-testid="media-below-block">
      <div className="compact">
        <div className="copy">
          <div className="masthead-row">
            <div className="masthead">
              <span className="dot" />
              media
            </div>
          </div>
          <div className="title headline">{media.title}</div>
          {subtitle !== null && (
            <div className="notif-subtitle-row">
              <span className="notif-subtitle">{subtitle}</span>
            </div>
          )}
          <div className="media-body">
            <span className="media-art big" aria-hidden="true">
              ♪
            </span>
            <div className="grow">
              <div className="media-transport">
                <button
                  type="button"
                  className="mr-btn"
                  aria-label="Previous track"
                  onClick={() => onCommand?.("previous")}
                >
                  <span className="mr-glyph prev" aria-hidden="true" />
                </button>
                <button
                  type="button"
                  className="mr-btn primary"
                  aria-label={media.playing ? "Pause" : "Play"}
                  onClick={() => onCommand?.("playPause")}
                >
                  <span
                    className={`mr-glyph ${media.playing ? "pause" : "play"}`}
                    aria-hidden="true"
                  />
                </button>
                <button
                  type="button"
                  className="mr-btn"
                  aria-label="Next track"
                  onClick={() => onCommand?.("next")}
                >
                  <span className="mr-glyph next" aria-hidden="true" />
                </button>
                {/* the shipped `.media-bar`/`.media-bar-fill` progress
                    indicator (idle-peek.css), reused verbatim — `scaleX`
                    transform (compositor-only, same discipline
                    `.ttl-fill` uses), never `width`. The extra 6px
                    inline margin matches the mock exactly (on top of
                    `.media-transport`'s own 6px flex gap) — it groups
                    the three transport buttons visually apart from the
                    bar+time pair, a nuance the shipped one-icon
                    `idle-peek.css` context never needed. */}
                <span className="media-bar" style={{ marginLeft: 6 }}>
                  <span
                    className="media-bar-fill"
                    style={{
                      transform: `scaleX(${progressPct / 100})`,
                      ...(discontinuity ? { transition: "none" } : {}),
                    }}
                  />
                </span>
                <span className="media-time">{formatElapsed(clampedElapsedMs)}</span>
              </div>
              {showExpandedDetail && (
                <>
                  <div className="media-scrub">
                    <span className="fill" style={{ width: `${progressPct}%` }} />
                    <span className="knob" style={{ left: `${progressPct}%` }} />
                  </div>
                  <div className="media-times">
                    <span>{formatElapsed(clampedElapsedMs)}</span>
                    <span>{durationDisplay}</span>
                  </div>
                </>
              )}
            </div>
          </div>
          {showExpandedDetail && queue.length > 0 && (
            <div className="queue-list">
              {/* anonymous positional queue slots (1..n, display order
                  only) — same reasoning PositionBar.tsx's own segment row
                  documents, index is the only identity a queue preview
                  has. */}
              {queue.map((item, i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: see comment above
                <div className="queue-row" key={i}>
                  <span className="n">{i + 1}</span>
                  {item.title}
                  <span className="who">{item.artist}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
