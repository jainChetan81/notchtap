import { convertFileSrc } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useState } from "react";
import { NOTCHTAP_EASE } from "../animationTiming";
import type { EventSignal, EventType, LivePillVariant, Priority } from "../lib/presentation";
import type { EspnMeta, SlotState, SourceKind } from "../useSlotState";
import { Manifest } from "./Manifest";
import { Stamp } from "./Stamp";
import type { Detail } from "./StatusRailCard";
import { TtlBar } from "./TtlBar";

// Serves ALL non-news origins: `origin` is the five-value `SourceKind`
// union. An exhaustive per-origin lookup (not a string-equality ternary)
// makes a future `SourceKind` addition a compile error here. "manual" is
// the `/notify` CLI push path, hence "cli"; "news" never reaches this
// branch (the `news` boolean routes it elsewhere), so its entry is a
// defensive fallback, never actually read.
// Cap on pair count: the worst-case `.detail-facts` stack stays a
// knowable height, not an unbounded one. Chosen well above every
// fixture's real usage (3 pairs).
// Exported so AgentBoard.tsx's fact-pill assembly caps against the
// SAME limit rather than a second hand-copied literal.
export const MAX_VISIBLE_DETAIL_PAIRS = 4;

// The three tone classes `manifest.css` implements for a fact pill
// (`.fact-pill.tone-accent`/`.tone-danger`/`.tone-safe`). No tone =
// the plain neutral pill — the generic branch's own look.
export type FactTone = "accent" | "danger" | "safe";

// A fact's optional trailing qualifier — the `.fp-tag` span (e.g.
// `Tool rm DESTRUCTIVE`). A tag carries its own tone; a fact's tone
// travels with the tag and wins over the call-level tone.
export type FactTag = { text: string; tone: FactTone };

// A `Detail` (the wire's flat `{label, value}` pair) plus that optional
// tag. Every existing `Detail` is already a valid `Fact` — the tag is
// synthesized by a caller with real domain knowledge of its own facts
// (`AgentBoard.tsx`), never carried on the wire.
export type Fact = Detail & { tag?: FactTag };

// The shared fact-pill renderer — one `.fact-pill` per label/value
// pair, used by BOTH the generic branch's `liveVisibleDetails` and the
// agent `AgentHeroCard` — one implementation, not two.
// `fp-label` is always shown: a wire `Detail` pair carries only
// `{label, value}`, no "omit the label" signal; that judgment belongs
// to a caller with real domain knowledge of its own facts.
// `tone` is the call-level tone every untagged pill in the call takes
// (`null` = neutral, which the generic branch passes); a fact with its
// own `tag` uses that tag's tone instead — so a danger-tagged pill
// inside an otherwise accent-toned state still reads danger.
function renderFactPills(facts: Fact[], tone: FactTone | null = null) {
  if (facts.length === 0) {
    return null;
  }
  return (
    <div className="detail-facts">
      {facts.map((fact, index) => {
        const pillTone = fact.tag?.tone ?? tone;
        return (
          <span
            // biome-ignore lint/suspicious/noArrayIndexKey: index is a tie-breaker only, not the primary key — a fresh `details` array from the wire each render, never locally reordered, so position is stable; this is what keeps two facts sharing the same label/value pair from colliding on an otherwise-identical key.
            key={`${fact.label}:${fact.value}:${index}`}
            className={`fact-pill${pillTone !== null ? ` tone-${pillTone}` : ""}`}
          >
            <span className="fp-label">{fact.label}</span>
            {fact.value}
            {fact.tag !== undefined && <span className="fp-tag">{fact.tag.text}</span>}
          </span>
        );
      })}
    </div>
  );
}

const GENERIC_MASTHEAD_KICKER: Record<SourceKind, string> = {
  manual: "cli",
  football: "football",
  news: "news",
  // agent-originated cards (`SourceKind::Agent`) get their own kicker
  // label, same table-driven discipline as every other origin here.
  agent: "agent",
};

// The whole non-live-match content fragment (compact + manifest +
// ttl-bar together) — hence NOT named `CompactBody`: `.compact` is
// already a load-bearing CSS class inside it. The block's free
// variables arrive as props, not re-derived here. `slot` is narrowed
// to the "showing" variant because the block reads many `slot.*`
// fields that only exist on that variant — narrowing at the prop
// boundary avoids an unreadable per-field prop list.
export function NotificationBody({
  news,
  slot,
  newsCategory,
  newsAge,
  bodyContent,
  expanded,
  liveVisibleDetails,
  hovered,
}: {
  news: boolean;
  slot: Extract<SlotState, { state: "showing" }>;
  newsCategory: string | null;
  newsAge: string | null;
  bodyContent: ReactNode;
  expanded: boolean;
  liveVisibleDetails: Detail[];
  hovered: boolean;
}) {
  return (
    <>
      <div className="compact">
        <div className="copy">
          {news ? (
            // News layout stays screenshot-faithful (masthead,
            // headline, WIRE stamp, news-shade, track); the Stamp
            // badge sits inline with the masthead (`.masthead-row`),
            // and age lives in the plain `.notif-time-inline` slot.
            // The compact row carries exactly one time expression;
            // the expanded Manifest keeps its own "published HH:MM"
            // segment.
            <>
              <div className="masthead-row">
                <div className="masthead">
                  <span className="dot" />
                  {slot.source ?? "RSS"}
                </div>
                <Stamp priority={slot.priority} signal={slot.signal} eventType={slot.eventType} />
              </div>
              <div className="title headline">{slot.title}</div>
              {(newsCategory !== null || newsAge !== null) && (
                <div className="notif-meta-row">
                  {newsCategory !== null && (
                    <span className="chip chip-category">{newsCategory}</span>
                  )}
                  {newsAge !== null && <span className="notif-time-inline">{newsAge}</span>}
                </div>
              )}
            </>
          ) : (
            // The generic branch's header converges onto the same
            // masthead-row/`.title.headline` markup news uses; the
            // kicker reads the origin-derived label instead of a
            // source name. The subtitle row and the full-width body
            // stay generic-only — news carries neither. Detail pairs
            // render compactly here, expanded or not — metadata
            // belongs in the compact card, never duplicated into the
            // expanded panel (Manifest renders none).
            <>
              <div className="masthead-row">
                <div className="masthead">
                  <span className="dot" />
                  {GENERIC_MASTHEAD_KICKER[slot.origin]}
                </div>
                <Stamp priority={slot.priority} signal={slot.signal} eventType={slot.eventType} />
              </div>
              <div className="title headline">{slot.title}</div>
              {slot.subtitle !== null && (
                <div className="notif-subtitle-row">
                  <span className="notif-subtitle">{slot.subtitle}</span>
                </div>
              )}
              {/* an empty body (e.g. an agent push with no body text)
                must not leave a blank `.notif-body` node in the card. */}
              {slot.body.trim() !== "" && <div className="notif-body">{bodyContent}</div>}
              {renderFactPills(liveVisibleDetails.slice(0, MAX_VISIBLE_DETAIL_PAIRS))}
            </>
          )}
        </div>
        {!expanded && (
          <div className="compact-hint">
            <kbd>⌃⇧N</kbd> more
          </div>
        )}
      </div>
      <Manifest
        title={slot.title}
        body={slot.body}
        eventType={slot.eventType}
        expanded={expanded}
        hasLink={slot.link !== null}
      />
      {/* Last in DOM order within .below-block — the bar is the
        card's floor, absolutely positioned to its bottom edge
        (styles.css), clipped to the rounded corners by
        .below-block's own overflow: hidden. Renders the queue
        segmentation itself from `total`/`done`, folded into this
        same floor bar. */}
      <TtlBar
        key={slot.id}
        slotId={slot.id}
        ttlMs={slot.ttlMs}
        remainingMs={slot.remainingMs}
        // TTL hover-pause: this bar only mounts while `showing`, so
        // `hovered` alone (the live cursor signal) is exactly "is
        // THIS card hovered right now" — no extra gating.
        hoverPaused={hovered}
        total={slot.queueTotal}
        done={slot.queueDone}
      />
    </>
  );
}

// The Agent Board's primary-session hero, rendered through the SAME
// masthead-row/Stamp/`.title.headline`/`.notif-subtitle-row`/fact-pill
// shapes the generic branch uses.
//
// Deliberately does NOT take a `slot`/`AgentSessionView` — every field
// arrives pre-computed as a plain primitive, so this file needs no
// agent-specific type imports; AgentBoard.tsx (the one place that
// knows about sessions/runtimes) does that translation and calls this
// directly. It does NOT render Manifest/TtlBar/`.compact-hint` —
// notification chrome that doesn't apply to a persistent status board
// with no TTL and its own hover-expand meaning.
//
// `dotKey`/`pulse` intentionally stay primitives too — AgentBoard.tsx
// remains the one place that renders the actual `.agent-dot` span
// (keyed on state, `large`/`pulse` classes), preserving the dot's
// bounded-pulse animation contract.
//
// `factsTone` is the call-level pill tone — `"danger"` for the two
// alarm states, `"accent"` for every other. Deliberately NOT defaulted:
// the generic branch passes no tone (neutral pills) and the hero always
// passes one, so the two looks can't silently converge.
export function AgentHeroCard({
  dotKey,
  pulse,
  title,
  subtitle,
  body,
  priority,
  facts,
  factsTone,
}: {
  dotKey: string;
  pulse: boolean;
  title: string;
  subtitle: string | null;
  body: string | null;
  priority: Priority;
  facts: Fact[];
  factsTone: FactTone;
}) {
  return (
    <div className="compact">
      <div className="copy">
        <div className="masthead-row">
          <div className="masthead">
            <span
              key={dotKey}
              className={`agent-dot large${pulse ? " pulse" : ""}`}
              aria-hidden="true"
            />
            <span className="agent-runtime-tick" aria-hidden="true" />
            {GENERIC_MASTHEAD_KICKER.agent}
          </div>
          <Stamp priority={priority} signal="generic" eventType="agent_event" />
        </div>
        <div className="title headline">{title}</div>
        {subtitle !== null && (
          <div className="notif-subtitle-row">
            <span className="notif-subtitle">{subtitle}</span>
          </div>
        )}
        {body !== null && body.trim() !== "" && <div className="notif-body">{body}</div>}
        {renderFactPills(facts, factsTone)}
      </div>
    </div>
  );
}

// Crest source is a filesystem path on the wire, never a ready
// `asset://` URL, so every render must go through `convertFileSrc`
// itself. `onError` is defense in depth against a cache entry gone
// stale on disk; the `broken` flag is deliberately sticky (not
// re-tried) so a permanently-404ing path doesn't flash forever.
function Crest({ abbrev, path }: { abbrev: string; path: string | null }) {
  const [broken, setBroken] = useState(false);
  const src = !broken && path !== null ? convertFileSrc(path) : null;
  return (
    <span className="crest">
      {src !== null ? <img src={src} alt="" onError={() => setBroken(true)} /> : abbrev}
    </span>
  );
}

// Score-roll timing. Deliberately LOCAL literals rather than
// animationTiming.ts tokens — that file single-sources values with a
// CSS or cross-component counterpart that must stay in lockstep, and
// this roll has exactly one consumer, no CSS twin. The delay lets the
// goal celebration's first beat land first: 120 + 360 = 480ms inside
// the 1240ms celebration window (choreography.css).
const SCORE_ROLL_S = 0.36;
const SCORE_ROLL_DELAY_S = 0.12;

// One side's score digit, as a single-digit odometer. The clip span
// is a fixed-height `overflow: hidden` box; the inner `motion.span`
// is KEYED ON THE VALUE: React only remounts (and AnimatePresence
// only animates) when the number genuinely changes. Everything else
// that re-renders this card — the clock pill, a same-slot re-emit
// with an unchanged scoreline — reuses the keyed child and rolls
// nothing, so no `.rotation-swap` style off-switch is needed.
// `initial={false}` renders a card mounting with a score at rest;
// `mode="popLayout"` takes the outgoing digit out of flow so the two
// overlap inside the clip; the percentage translate is
// compositor-only.
function ScoreDigit({ value }: { value: number }) {
  return (
    <span className="score-digit">
      <AnimatePresence initial={false} mode="popLayout">
        <motion.span
          key={value}
          className="score-digit-roll"
          initial={{ y: "100%" }}
          animate={{ y: 0 }}
          exit={{ y: "-100%" }}
          transition={{ duration: SCORE_ROLL_S, ease: NOTCHTAP_EASE, delay: SCORE_ROLL_DELAY_S }}
        >
          {value}
        </motion.span>
      </AnimatePresence>
    </span>
  );
}

// The live-match football card, rendered through the shared
// masthead/stamp/accent-stripe template. `title` is `slot.body`
// verbatim (e.g. "Goal — K. Havertz 78'"), NOT `slot.title` (the
// matchup string is already redundant with the score-row's own
// crests+digits).
//
// No subtitle, no `.notif-body`, no fact pills: the wire meta
// carries only the flat body string, plus a Clock detail and an
// aggregate Cards tally — the SAME two facts the score-row already
// shows verbatim (the `clock-pill` chip, the `cards-line` block),
// so a pill here would be pure duplication. No `<Manifest>`/
// `<TtlBar>`: a sticky recurring presence; `<Stamp>` is the real
// per-signal one (Card/Off/Foul/Offside/VAR/Sub/Break/Final via
// `stampFor`/`SIGNAL_STAMPS`).
//
// The score-row (league/live-state/clock chips, crests, rolling
// score digits, optional cards-line) sits ADDITIVE below
// `.title.headline`, not folded into the generic template shape.
//
// The `.event-line` icon+tint does NOT carry over onto
// `.title.headline`: that was a flex row built for an icon beside
// text, `.title.headline` is a line-clamped text block with no icon
// slot, and `slot.body` already names the event. The goal/red-card
// celebration on `.card-assembly` is a shell-level effect,
// independent of whichever content template sits underneath it.
// The score-block markup is extracted so the crossbar variant's
// plain (`stacked`) block below can reuse it — `stacked` toggles
// the ONE class difference (`score-block stacked`); every other
// prop and every line is identical to the primary block.
function ScoreBlockContent({
  stacked,
  liveEspn,
  pillVariant,
  pillLabel,
  cardsClean,
}: {
  stacked: boolean;
  liveEspn: EspnMeta;
  pillVariant: LivePillVariant;
  pillLabel: string;
  cardsClean: boolean;
}) {
  return (
    <div className={stacked ? "score-block stacked" : "score-block"}>
      <div className="sc-head">
        <span className="chip chip-league">{liveEspn.league}</span>
        <span className={`chip chip-live${pillVariant === "live" ? "" : ` ${pillVariant}`}`}>
          {/* the dot stays MOUNTED in every variant, including
              `final` — it leaves by fading to `opacity: 0`
              (live-scorecard.css `.chip-live.final .live-dot`), in
              step with the chip's own colour morph, instead of
              unmounting a frame early. */}
          <span className="live-dot" />
          {pillLabel}
        </span>
        <span className="chip clock-pill">{liveEspn.clock}</span>
      </div>
      <div className="score-row">
        <div className="side">
          <Crest abbrev={liveEspn.homeAbbrev} path={liveEspn.homeCrest} />
        </div>
        <span className="score">
          <ScoreDigit value={liveEspn.homeScore} />
          <span className="dash">–</span>
          <ScoreDigit value={liveEspn.awayScore} />
        </span>
        <div className="side">
          <Crest abbrev={liveEspn.awayAbbrev} path={liveEspn.awayCrest} />
        </div>
      </div>
      {!cardsClean && (
        <div className="cards-line">
          {liveEspn.homeAbbrev} {liveEspn.homeCards[0]}Y{liveEspn.homeCards[1]}R ·{" "}
          {liveEspn.awayAbbrev} {liveEspn.awayCards[0]}Y{liveEspn.awayCards[1]}R
        </div>
      )}
    </div>
  );
}

// One ADDITIONAL live match for the crossbar variant's stacked second
// block — every field caller-computed, same "no domain-specific
// derivation inside this component" discipline as `cardsClean`
// (StatusRailCard.tsx computes it, FootballHeroCard just renders it).
export interface SecondaryMatch {
  liveEspn: EspnMeta;
  pillVariant: LivePillVariant;
  pillLabel: string;
  cardsClean: boolean;
}

export function FootballHeroCard({
  title,
  priority,
  signal,
  eventType,
  liveEspn,
  pillVariant,
  pillLabel,
  cardsClean,
  // The "crossbar" persistent variant: two stacked score blocks, no
  // event headline, no TTL bar when football is selected and a match
  // is live. Deliberately NOT a separate `crossbar` boolean — what
  // `title` the caller passes plus whether this list is non-empty
  // expresses it; a redundant flag would restate caller-controlled
  // information. Defaults to empty, so every existing caller renders
  // byte-identical.
  //
  // Nothing on the wire currently surfaces more than one live match:
  // `StatusState.football.live` (status.rs) is a single `Option`, and
  // `poller.rs`'s snapshot collapses to "the first in-play match
  // wins" before it reaches the wire. Surfacing a second
  // simultaneously-live match needs a rust-side wire change outside
  // this file's scope.
  secondaryMatches = [],
}: {
  title: string;
  priority: Priority;
  signal: EventSignal;
  eventType: EventType;
  liveEspn: EspnMeta;
  pillVariant: LivePillVariant;
  pillLabel: string;
  cardsClean: boolean;
  secondaryMatches?: SecondaryMatch[];
}) {
  return (
    <div className="compact">
      <div className="copy">
        <div className="masthead-row">
          <div className="masthead">
            <span className="dot" />
            {GENERIC_MASTHEAD_KICKER.football}
          </div>
          <Stamp priority={priority} signal={signal} eventType={eventType} />
        </div>
        <div className="title headline">{title}</div>
        {/* The score block is ONE wrapper carrying the 10px gap off
            `.title.headline` that the chips row otherwise butts
            against at 0px (live-scorecard.css). */}
        <ScoreBlockContent
          stacked={false}
          liveEspn={liveEspn}
          pillVariant={pillVariant}
          pillLabel={pillLabel}
          cardsClean={cardsClean}
        />
        {secondaryMatches.map((match) => (
          <ScoreBlockContent
            key={`${match.liveEspn.league}-${match.liveEspn.homeAbbrev}-${match.liveEspn.awayAbbrev}`}
            stacked={true}
            liveEspn={match.liveEspn}
            pillVariant={match.pillVariant}
            pillLabel={match.pillLabel}
            cardsClean={match.cardsClean}
          />
        ))}
      </div>
    </div>
  );
}
