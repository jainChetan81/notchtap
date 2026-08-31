import { convertFileSrc } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useState } from "react";
import type { EspnMeta, SlotState, SourceKind } from "../hooks/useSlotState";
import { NOTCHTAP_EASE } from "../lib/constants";
import type { EventSignal, EventType, LivePillVariant, Priority } from "../lib/presentation";
import { Manifest } from "./Manifest";
import { Stamp } from "./Stamp";
import type { Detail } from "./StatusRailCard";
import { TtlBar } from "./TtlBar";

export const MAX_VISIBLE_DETAIL_PAIRS = 4;

export type FactTone = "accent" | "danger" | "safe";

export type FactTag = { text: string; tone: FactTone };

export type Fact = Detail & { tag?: FactTag };

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

const GENERIC_MASTHEAD_KICKER = {
  manual: "cli",
  football: "football",
  news: "news",
  agent: "agent",
} satisfies Record<SourceKind, string>;

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
      <TtlBar
        key={slot.id}
        slotId={slot.id}
        ttlMs={slot.ttlMs}
        remainingMs={slot.remainingMs}
        hoverPaused={hovered}
        total={slot.queueTotal}
        done={slot.queueDone}
      />
    </>
  );
}

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

function Crest({ abbrev, path }: { abbrev: string; path: string | null }) {
  const [broken, setBroken] = useState(false);
  const src = !broken && path !== null ? convertFileSrc(path) : null;
  return (
    <span className="crest">
      {src !== null ? <img src={src} alt="" onError={() => setBroken(true)} /> : abbrev}
    </span>
  );
}

const SCORE_ROLL_S = 0.36;
const SCORE_ROLL_DELAY_S = 0.12;

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
