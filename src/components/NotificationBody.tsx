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

// 2026-07-24 (declutter fix): the generic branch serves ALL non-news
// origins — `origin` is the five-value `SourceKind` union
// (`src/useSlotState.ts`'s `SOURCE_KINDS`). An exhaustive per-origin
// lookup (not a string-equality ternary) keeps a future SourceKind
// addition a compile error here until this table is updated, same
// discipline as lib/presentation.ts's own exhaustive tables. "manual" is
// the `/notify` CLI push path, hence "cli"; "news" never reaches this
// branch (the `news` boolean above routes it to the news branch
// instead), so its entry is a defensive fallback, never actually read.
// plan 137 (spec §7/§12): the "cmux" entry is gone — `SourceKind` no
// longer has that variant (superseded by "agent").
// M13 (layout overflow in the fixed 500x300 window): `liveVisibleDetails`
// used to render every pair the payload carried, uncapped — a worst-case
// server push (up to ~8 detail pairs, per the wire contract's own
// generous allowance) could grow `.compact` tall enough to push the
// TTL bar's floor strip out of the window (`.below-block`'s `overflow:
// hidden` just hard-crops instead of scrolling). Plan 169 moved each
// pair off a 2-line-clamped stacked block onto a single-line, ellipsis-
// truncating `.fact-pill` (manifest.css) — narrower per item, but still
// paired with this same cap on pair COUNT, so the worst case (many
// pills stacked in `.detail-facts`) stays a knowable height instead of
// an unbounded one. Chosen well above every existing fixture's real
// usage (3 pairs, the richest StatusRailCard.test.tsx case) so no
// legitimate payload is ever visibly truncated in practice.
// Exported (plan 169) so AgentBoard.tsx's own fact-pill assembly for the
// templated hero (session.details plus one synthesized elapsed fact)
// caps against the SAME limit, rather than a second hand-copied literal.
export const MAX_VISIBLE_DETAIL_PAIRS = 4;

// Plan 169 fidelity pass (2026-08-02): the three tone classes
// `manifest.css` actually implements for a fact pill (`.fact-pill.tone-
// accent`/`.tone-danger`/`.tone-safe`). A pill with NO tone is the plain
// neutral pill — that is the generic branch's own look and stays it.
export type FactTone = "accent" | "danger" | "safe";

// Plan 169 fidelity pass: a fact's optional trailing QUALIFIER — the
// mock's `.fp-tag` span (`prototype/agent-board.html`'s proposal
// fixtures: `Tool rm DESTRUCTIVE`, `Exit 1 ERROR`). Carries its own tone
// because a tag is exactly the thing that "deserves a color" (plan 169's
// Target wording): a tagged fact's tone travels WITH the tag rather than
// being re-derived by the caller, and it wins over whatever call-level
// tone the rest of the pills in that call carry.
export type FactTag = { text: string; tone: FactTone };

// A `Detail` (the wire's flat `{label, value}` pair) plus that optional
// tag. Every existing `Detail` is already a valid `Fact` — the tag is
// synthesized by a caller with real domain knowledge of its own facts
// (`AgentBoard.tsx`), never carried on the wire.
export type Fact = Detail & { tag?: FactTag };

// Plan 169 (steps 2-3): the shared fact-pill renderer — one `.fact-pill`
// per label/value pair, replacing the old stacked `.detail-label`/
// `.detail-value` divs (manifest.css). Used by BOTH the generic branch's
// own `liveVisibleDetails` below and the agent-aware `AgentHeroCard`
// further down — one rendering implementation, not two hand-copied
// ones, per this plan's own "no parallel pill system" premise.
// `fp-label` is always shown here: a wire `Detail`/`AgentDetail` pair
// carries only `{label, value}`, no signal for "this value is
// self-explanatory, omit the label" — that per-fact judgment call
// belongs to a caller with real domain knowledge of its own facts
// (e.g. plan 170's football scorer line), not this generic renderer.
// `tone` is the CALL-level tone every untagged pill in the call takes
// (`null` = the plain neutral pill, which is what the generic branch
// passes); a fact carrying its own `tag` uses that tag's tone instead,
// exactly as the mock's fixtures do (a danger-tagged pill inside an
// otherwise accent-toned state would still read danger).
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
  weather: "weather",
  news: "news",
  // plan 135: agent-originated cards (`SourceKind::Agent`) get their own
  // kicker label, same table-driven discipline as every other origin here.
  agent: "agent",
};

// plan 120: extracted verbatim from StatusRailCard.tsx's JSX (`:711-838`
// at 2a840c4) — the whole non-live-match content fragment (compact +
// manifest + ttl-bar together, hence NOT named `CompactBody`: `.compact`
// is already a load-bearing CSS class inside it that a component named
// `CompactBody` would collide with in the reader's head). Every free
// variable the block read is now a prop, not re-derived here (the
// lower-risk "moved as-is" shape, matching `LiveMatchScorecard`).
// `slot` is narrowed to the "showing" variant (not the bare `SlotState`
// union) because the block reads many `slot.*` fields that only exist on
// that variant — narrowing at the prop boundary avoids an unreadable
// individual-field prop list.
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
            // plan 092 (item 19 + 080 carry-forward): the shipped
            // news layout stays screenshot-faithful (masthead,
            // headline, WIRE stamp, news-shade, track) — only the
            // Stamp badge's position (now inline with the
            // masthead, `.masthead-row`) and the pills' visual
            // vocabulary (chip-converged, item 10) change. Age
            // moves out of the meta row entirely into the plain
            // `.notif-time-inline` slot (Decision 5 — same
            // ageLabel computation/thresholds, new location).
            // plan 110 (Step C): the redundant `.pub-meta`
            // "published HH:MM" node is gone — the compact row
            // now carries exactly one time expression (the
            // relative age above). The expanded Manifest's own
            // "published HH:MM" segment is untouched (its own
            // pinned test lives in StatusRailCard.test.tsx).
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
            // 2026-07-24 (compact/expanded declutter): the generic
            // branch's header converged onto the SAME masthead-row/
            // `.title.headline` markup news uses above — the kicker
            // reads the origin-derived label (`GENERIC_MASTHEAD_KICKER`
            // above) instead of a source name, and the old separate
            // `.notif-header-row`/`.notif-title`/`.notif-header-badges`/
            // `chip-cmux` "Agent" chip are gone (the kicker already said
            // cmux for that origin, so the chip was pure duplication;
            // plan 137 renamed that kicker entry to "agent").
            // Subtitle row (plan 035's `subtitle`,
            // surfaced in compact) and the full-width body stay
            // generic-only — news never carries either. Detail pairs
            // (plan 035's `details` channel) now always render
            // compactly here, expanded or not — metadata belongs in
            // the compact card, never duplicated into the expanded
            // panel (see Manifest.tsx, which no longer renders them at
            // all).
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
      {/* plan 100: last in DOM order within .below-block — the bar
        is the card's floor, absolutely positioned to its bottom
        edge (styles.css), clipped to the rounded corners by
        .below-block's own overflow: hidden.
        stories merge (2026-07-24): the standalone `<Track>` queue slider that used to sit
        inside `.compact` above is gone — the two strips read as a double
        border. TtlBar now takes `total`/`done` directly and renders the
        queue segmentation itself, folded into this same floor bar. */}
      <TtlBar
        key={slot.id}
        slotId={slot.id}
        ttlMs={slot.ttlMs}
        remainingMs={slot.remainingMs}
        // plan 093: TTL hover-pause — this bar only ever mounts
        // while `showing`, so `hovered` alone (the live cursor
        // signal) is exactly "is THIS card hovered right now,"
        // no extra gating needed.
        hoverPaused={hovered}
        total={slot.queueTotal}
        done={slot.queueDone}
      />
    </>
  );
}

// Plan 169 (step 4): the Agent Board's primary-session hero — the
// agent-aware branch this plan adds "alongside the existing generic/news
// branches" (Repo conventions), rendering through the SAME masthead-row/
// Stamp/`.title.headline`/`.notif-subtitle-row`/`.notif-body`/fact-pill
// shapes the generic branch above uses, in place of the old bespoke
// `.agent-board-primary-head` block (AgentBoard.tsx, pre-169).
//
// Deliberately does NOT take a `slot`/`AgentSessionView` — every field
// arrives pre-computed as a plain primitive (a title string, a nullable
// subtitle/body string, a `Detail[]` fact list, a dot key + pulse flag)
// so this component needs no agent-specific type import
// (`AgentSessionView`/`AgentRuntime`/`AgentSessionState`) — AgentBoard.tsx
// (the one place that actually knows about sessions/runtimes) does that
// translation and calls this directly, per the plan's own "whichever
// keeps NotificationBody.tsx from needing agent-specific imports it
// doesn't otherwise need" guidance. It does NOT render Manifest/TtlBar/
// `.compact-hint` — those are notification-specific chrome (TTL/queue/
// expand-to-read-more) that don't apply to a persistent status board
// with no TTL and its own hover-expand meaning; only the `.compact`
// masthead/title/subtitle/body/fact-pill shape is reused.
//
// `dotKey`/`pulse` intentionally stay primitive too (not a pre-rendered
// dot element) — AgentBoard.tsx is still the one place that renders the
// actual `.agent-dot` span (keyed on state, `large`/`pulse` classes),
// preserving the Boundary that this plan changes markup/CSS classes
// around the dot, never its own bounded-pulse animation contract.
//
// `factsTone` (plan 169 fidelity pass) is the call-level pill tone —
// `"danger"` for the two alarm states, `"accent"` for every other state,
// mirroring the mock's own per-state fixtures (`prototype/agent-board
// .html`, proposal section). Deliberately NOT defaulted here: the
// generic branch above passes no tone at all (neutral pills), and the
// agent hero always passes one, so making it explicit keeps the two
// looks from silently converging.
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

// plan 084: the recurring live-match scorecard's crest — a filesystem path
// on the wire (083 workstream a), never a ready `asset://` URL, so every
// render must go through `convertFileSrc` itself. `onError` is defense in
// depth for a cache entry that's gone stale on disk between poll and
// render; the `broken` flag is deliberately sticky (not re-tried) so a
// permanently-404ing path doesn't flash between the two states forever.
// Plan 170: ported verbatim from the now-deleted `LiveMatchScorecard.tsx`
// into `FootballHeroCard`'s additive score-row below — same behavior,
// only its home file changed.
function Crest({ abbrev, path }: { abbrev: string; path: string | null }) {
  const [broken, setBroken] = useState(false);
  const src = !broken && path !== null ? convertFileSrc(path) : null;
  return (
    <span className="crest">
      {src !== null ? <img src={src} alt="" onError={() => setBroken(true)} /> : abbrev}
    </span>
  );
}

// plan 151 (item B): the score odometer's own timing. Deliberately LOCAL
// literals rather than new animationTiming.ts tokens — that file
// single-sources values with a CSS or cross-component counterpart that
// must stay in lockstep (see its header), and this roll has exactly one
// consumer and no CSS twin. The delay lets the goal celebration's own
// first beat land first: 120 + 360 = 480ms, well inside the 1240ms
// celebration window (choreography.css), so the digit finishes rolling
// while the burst is still playing rather than after it.
const SCORE_ROLL_S = 0.36;
const SCORE_ROLL_DELAY_S = 0.12;

// plan 151 (item B): one side's score digit, as a single-digit odometer.
//
// The payload used to be the one thing on this card that did NOT move —
// `goal-overshoot`/`goal-burst`/the ripple all celebrate AROUND a number
// that simply swapped between frames. The clip span is a fixed-height
// `overflow: hidden` box (live-scorecard.css `.score-digit`); the inner
// `motion.span` is KEYED ON THE VALUE, which is the whole restraint
// guard: React only remounts (and AnimatePresence only animates) when the
// number genuinely changes. Everything else that re-renders this card —
// most notably the once-a-minute clock pill, but also any same-slot
// rotation re-emit carrying an unchanged scoreline — reuses the same
// keyed child and rolls nothing. That is why this needs no `.rotation-swap`
// style off-switch (news-category.css): value-keying already expresses
// "only on a real change", and it can't be fooled by a re-emit.
//
// `initial={false}` on the AnimatePresence means a card mounting with a
// score already on it renders that score at rest — the roll is reserved
// for a change that happens while the card is on screen. `mode="popLayout"`
// takes the outgoing digit out of flow so the two digits overlap inside
// the clip instead of the box briefly widening to hold both. The
// percentage translate is compositor-only (no layout per frame), same
// discipline as `.ttl-fill`/`.media-bar-fill`.
// Plan 170: ported verbatim, same reasoning as `Crest` above.
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

// Plan 170: the live-match football card, rendered through the shared
// masthead/stamp/accent-stripe template (`AgentHeroCard`'s own precedent,
// plan 169) instead of `LiveMatchScorecard.tsx`'s now-deleted bespoke
// `.notif-block` layout. `title` is `slot.body` verbatim (e.g. "Goal — K.
// Havertz 78'"), NOT `slot.title` (`matchup()`'s "UCL: ARS 1–1 PSG" —
// already redundant with the score-row's own crests+digits below).
//
// No subtitle, no `.notif-body`, no fact pills. Football's wire meta
// (`poller.rs`'s `diff_match`) carries only the one flat body string,
// plus a Clock detail and an aggregate Cards-tally detail when
// `espn_live_card` is on — the SAME two facts the kept score-row already
// shows verbatim (the `clock-pill` chip, the `cards-line` block below),
// and no scorer/assist/booking-level data beyond that exists anywhere on
// the wire. A fact pill fed from `slot.details` here would therefore be
// pure duplication, not new information — plan 170's Target section has
// the full accounting (corrected twice during that plan's own dispatch:
// first for the aspirational fact-pill design, then for an incorrect
// "meta is always empty" claim about exactly this data). Does NOT render
// `<Manifest>`/`<TtlBar>` either (unlike the generic/news branches in
// `NotificationBody` above) — same "no TTL bar for a sticky recurring
// presence, no generic Stamp redundant with the live chip" rule
// `LiveMatchScorecard.tsx` originally documented; `<Stamp>` here is the
// real per-signal one (Card/Off/Foul/Offside/VAR/Sub/Break/Final via
// `stampFor`/`SIGNAL_STAMPS`), not the redundant reference that doc was
// about.
//
// The score-row (league/live-state/clock chips, crests, rolling score
// digits, optional cards-line) is a kept ADDITIVE block below
// `.title.headline` — not folded into the generic template's shape, same
// precedent as Agent Board's queue-rows (plan 169).
//
// The old component's `.event-line` icon+tint (`eventPresentation` from
// lib/presentation.ts) does NOT carry over onto `.title.headline`: that
// was a flex row built for an icon beside text, `.title.headline` is a
// line-clamped text block with no icon slot, and `slot.body`'s own text
// already names the event ("Goal —"/"Penalty - Scored —"/"Own Goal —").
// The goal/red-card celebration on `.card-assembly` is unaffected — a
// shell-level effect independent of whichever content template sits
// underneath it (StatusRailCard.tsx still computes it separately, off
// `footballKind`/`eventKindPresentationFor`, unchanged by this plan).
// Plan 171 (tab-notch redesign, slice G): the score-block markup,
// extracted verbatim from `FootballHeroCard`'s own former inline JSX so
// the crossbar variant's second (`stacked`) block below can reuse it
// instead of a second, drifting copy. `stacked` toggles the ONE class
// difference (`score-block` vs `score-block stacked`, live-scorecard.css)
// — every other prop and every line of markup is identical to what the
// primary block always rendered.
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
          {/* plan 151 (item A): the dot stays MOUNTED in every variant,
              including `final` — it leaves by fading to `opacity: 0`
              (live-scorecard.css `.chip-live.final .live-dot`) in step
              with the chip's own colour morph, instead of being
              unmounted and blinking out a frame before the colours
              change. Smallest structural change that lets it fade; the
              collapsed 5px+gap it leaves behind is animated in the same
              rule. */}
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

// Plan 171 (tab-notch redesign, slice G): one ADDITIONAL live match for
// the crossbar variant's stacked second block — every field caller-
// computed, same "no domain-specific derivation inside this component"
// discipline `cardsClean` already established for the primary match
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
  // Plan 171 (tab-notch redesign, slice G): spec section 7's football
  // bullet / the design source's own "crossbar" persistent variant
  // (prototypes/tab-notch-panel.html: "two stacked score-blocks, no
  // event headline, no TTL bar", reached via prefix+enter while football
  // is selected and a match is live). Deliberately NOT a separate
  // `crossbar` boolean — "crossbar-ness" is fully expressed by what
  // `title` string the caller passes (the event line, vs. e.g. "2
  // matches live") plus whether this list is non-empty; a redundant flag
  // would just restate information the caller already controls. Defaults
  // to empty, so every EXISTING caller (the shipped notification card)
  // renders byte-identical to before this slice.
  //
  // No real caller populates this yet: nothing on the wire currently
  // surfaces more than ONE live match at a time —
  // `StatusState.football.live` (status.rs) is a single `Option`, and
  // `poller.rs`'s own snapshot map (which DOES already track every
  // watched league) explicitly collapses to "the first in-play match
  // wins" before it ever reaches the wire. Surfacing a second
  // simultaneously-live match would need a rust-side wire change this
  // slice's own file scope (`NotificationBody.tsx` only) does not cover
  // — flagged here rather than improvised; Slice K's integration (or a
  // dedicated follow-up) is where that wire change belongs.
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
        {/* plan 170 (prototype-fidelity fix): the score block is ONE wrapper,
            matching prototype/football-card.html's `<div class="score-block">`
            — it carries the 10px gap off `.title.headline` that the chips
            row otherwise butts against at 0px (live-scorecard.css). */}
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
