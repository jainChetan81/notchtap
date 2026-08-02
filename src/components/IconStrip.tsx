// Plan 171 (tab-notch redesign, spec docs/superpowers/specs/2026-08-02-
// tab-notch-design.md section 6): the five neon icon-tabs, right-aligned
// inside the right flank, hidden entirely at rest and revealed only once
// the flank behind them has already painted black (the flank's own
// `.hovered` gate lives in the caller's CSS, not here — this component
// always renders all five icons; `icon-strip.css`'s `.hovered .icon-strip`
// rule is what makes them visible, matching `hover::icon_strip_rects`
// (src-tauri/src/hover.rs)'s own "present icons occupy space, absent ones
// collapse to zero width" contract exactly: this component renders every
// tab UNCONDITIONALLY, in the fixed strip order, and lets CSS width/
// opacity/scale transitions do the collapsing — never `display: none`,
// per spec section 6's own "must never jump mid-hover" rule.
//
// All five glyphs are original notchtap drawings (CLAUDE.md "naming";
// spec section 2 decision 13) — hand-authored paths, not a third-party
// icon set. Redraw freely; the shapes here are a first pass, not a
// locked asset.
import { type ReactNode, useId } from "react";

export type Tab = "agent" | "football" | "music" | "weather" | "news";

export const TAB_ORDER: readonly Tab[] = ["agent", "football", "music", "weather", "news"];

// Spec section 6's three-tier luminance scheme, uniform across all five
// icons: "hidden" never renders as `.is-present` at all (zero width,
// invisible — the strip's own baseline `.icon` rule), "present" is the
// dim 0.62-opacity tier ("present but idle — weather, quiet news" per
// the design source), "live" is full 1.0 opacity ("genuinely live —
// agent/match/audio"). Agent/football/music are only ever "hidden" or
// "live" in practice (spec section 6's table: each is present ONLY
// while genuinely live, so there is no "present but not live" state for
// them) — "present" mainly exists for weather (always present, rarely
// escalated) and news (dim while charging, escalating only once
// `newsCharged` below is separately true).
export type IconVisualState = "hidden" | "present" | "live";

export interface IconStripProps {
  agent: IconVisualState;
  football: IconVisualState;
  music: IconVisualState;
  weather: IconVisualState;
  news: IconVisualState;
  /** Spec section 8: 0..1 fill level, rising silently across the poll
   * cycle. Purely visual (a `scaleY` on the glyph's own interior
   * rectangle) — never glows, never implies `newsCharged`. */
  newsCharge: number;
  /** Spec section 8: the cycle has ended AND items are genuinely
   * waiting — the glyph goes to full weight and breathes coral ->
   * salmon until visited (selecting the news tab). Independent of
   * `news: IconVisualState` above (a news icon can be "present" —
   * dim, unread nothing pending — while `newsCharged` is false, or
   * "live"-equivalent-styled once charged; charged styling wins
   * visually regardless of the `news` tier passed in, matching the
   * mock's own `.icon.news.is-charged { opacity: 1 }` override, which
   * is declared AFTER (and so beats) the plain luminance tiers at
   * equal specificity). */
  newsCharged: boolean;
  /** Spec section 12 open question 5's shipped default ("ship both"):
   * the literal count badge alongside the ambient fill. `null` omits
   * the badge entirely (nothing waiting, or the count is unknown). */
  newsCount: number | null;
  selected: Tab | null;
  /** Fires on a click that lands on a `live`- or `present`-tier icon
   * (a `hidden` icon is not interactive — `pointer-events: none` in
   * CSS backs this up, this prop is the React-side mirror of that
   * rule for anyone testing the component directly). Deliberately
   * optional and deliberately just a callback, not a rust round-trip:
   * see plans/171-tab-notch-redesign.md's slice A note on the still-
   * open click-detection mechanism question — this component is
   * written to be correct under EITHER eventual answer. */
  onSelect?: (tab: Tab) => void;
}

const TAB_LABEL: Record<Tab, string> = {
  agent: "Agent",
  football: "Football",
  music: "Music",
  weather: "Weather",
  news: "News",
};

// CodeRabbit review fix (PR #13): an explicit `aria-label` overrides
// accessible-name computation from child content entirely, so the
// visually-rendered `.charge-count` badge (sighted-only) never reached
// assistive tech — a screen-reader user got no indication of the
// pending-item count sighted users see. Only the news tab has a count at
// all; every other tab (and news with no count) renders exactly
// `TAB_LABEL[tab]`, unchanged.
function iconAriaLabel(tab: Tab, newsCount: number | null): string {
  if (tab === "news" && newsCount !== null) {
    return `${TAB_LABEL[tab]}, ${newsCount} new`;
  }
  return TAB_LABEL[tab];
}

// Original notchtap glyphs, 18x18 viewBox (matching --icon-box), stroke-
// only where the mock's own drawings are stroke-only (agent, football,
// music, news) and one closed fill path for weather's cloud (the mock's
// own exception — "one closed cubic path"). `currentColor` throughout so
// the per-tab `--hue` custom property (icon-strip.css) drives both the
// stroke/fill AND, via the two stacked `drop-shadow`s on the parent
// `<svg>` wrapper in CSS, the glow — one value, never two to keep in
// sync (mirroring the mock's own "the whole point" comment on this).
function AgentGlyph(): ReactNode {
  return (
    <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <path
        d="M4 5 L9 9 L4 13"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path d="M10.5 13 H14" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}

export function FootballGlyph(): ReactNode {
  return (
    <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <circle cx="9" cy="9" r="6.5" stroke="currentColor" strokeWidth="1.4" />
      <path
        d="M9 9 L9 3.5 M9 9 L13.7 11.7 M9 9 L4.3 11.7"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
    </svg>
  );
}

function MusicGlyph(): ReactNode {
  return (
    <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <circle cx="6" cy="13" r="2.6" fill="currentColor" />
      <path
        d="M8.5 13 V4.5 L14 3 V6.5"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
      <path
        d="M8.5 4.5 L14 3 M8.5 7.2 L14 5.7"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
      />
    </svg>
  );
}

export function WeatherGlyph(): ReactNode {
  return (
    <svg viewBox="0 0 18 18" fill="currentColor" aria-hidden="true">
      <path d="M5.5 13.5 a3.2 3.2 0 0 1 -0.4 -6.38 a3.6 3.6 0 0 1 6.9 -1.5 a2.9 2.9 0 0 1 -0.3 7.88 z" />
    </svg>
  );
}

export function NewsGlyph({ charge }: { charge: number }): ReactNode {
  const clamped = Math.max(0, Math.min(1, charge));
  // A real, unique id per mount (React's useId) — not a hardcoded string.
  // Two NewsGlyphs in the same document (unlikely in the live app, real
  // in a test rendering two IconStrips side by side) would otherwise
  // collide: SVG `id`s are document-global, so the second glyph's
  // `clip-path` url() would silently resolve to the FIRST glyph's
  // clipPath element instead of its own.
  const clipId = useId();
  return (
    <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <rect x="3" y="2.5" width="12" height="13" rx="1.5" stroke="currentColor" strokeWidth="1.2" />
      {/* the ambient charge fill, section 8: a rect clipped to the page
          outline, scaleY off a bottom origin — never a height change
          (compositor-only, matching the eq bars/media bar discipline
          this app already uses everywhere else a fill level animates). */}
      <clipPath id={clipId}>
        <rect x="3" y="2.5" width="12" height="13" rx="1.5" />
      </clipPath>
      <rect
        className="charge"
        x="3"
        // CodeRabbit review fix (PR #13): this was `y="15.5"` — the rect's
        // own unscaled bounds (y 15.5 to 28.5) never overlapped the
        // clip region (y 2.5 to 15.5, matching the page outline above)
        // at ANY scaleY value, so the charge fill rendered invisible at
        // every charge level. `y="2.5"` matches the outline's own top so
        // the rect's full (scaleY(1)) extent exactly fills it; scaling
        // toward 0 around the bottom-anchored transformOrigin below
        // shrinks the visible portion upward from the bottom, the
        // liquid-filling-from-bottom effect the comment above describes.
        y="2.5"
        width="12"
        height="13"
        clipPath={`url(#${clipId})`}
        fill="currentColor"
        opacity="0.55"
        style={{ transform: `scaleY(${clamped})`, transformOrigin: "9px 15.5px" }}
      />
      <path
        d="M5.5 6 H12.5 M5.5 8.6 H10"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
      />
    </svg>
  );
}

function iconClass(tab: Tab, state: IconVisualState, selected: boolean, charged: boolean): string {
  const classes = [`icon ${tab}`];
  if (state !== "hidden") classes.push("is-present");
  if (state === "live") classes.push("is-live");
  if (selected) classes.push("is-selected");
  if (tab === "news" && charged) classes.push("is-charged");
  return classes.join(" ");
}

export function IconStrip({
  agent,
  football,
  music,
  weather,
  news,
  newsCharge,
  newsCharged,
  newsCount,
  selected,
  onSelect,
}: IconStripProps) {
  const states: Record<Tab, IconVisualState> = { agent, football, music, weather, news };
  return (
    <span className="icon-strip">
      {TAB_ORDER.map((tab) => {
        const state = states[tab];
        const isPresent = state !== "hidden";
        const isSelected = selected === tab;
        const isCharged = tab === "news" && newsCharged;
        return (
          <button
            key={tab}
            type="button"
            className={iconClass(tab, state, isSelected, isCharged)}
            aria-label={iconAriaLabel(tab, newsCount)}
            aria-pressed={isSelected}
            // A hidden icon is not a real control — spec section 6's own
            // "hidden AND opacity 0 AND pointer-events none" rule
            // (mirrored in CSS); `disabled` is the React/DOM-level twin
            // of that same rule, not a separate decision.
            disabled={!isPresent}
            onClick={isPresent ? () => onSelect?.(tab) : undefined}
          >
            {tab === "agent" && <AgentGlyph />}
            {tab === "football" && <FootballGlyph />}
            {tab === "music" && <MusicGlyph />}
            {tab === "weather" && <WeatherGlyph />}
            {tab === "news" && <NewsGlyph charge={newsCharge} />}
            {tab === "news" && newsCount !== null && (
              <span className="charge-count">{newsCount}</span>
            )}
          </button>
        );
      })}
    </span>
  );
}
