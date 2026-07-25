import type { ReactNode } from "react";
import type { SlotState, SourceKind } from "../useSlotState";
import { Manifest } from "./Manifest";
import { Stamp } from "./Stamp";
import type { Detail } from "./StatusRailCard";
import { TtlBar } from "./TtlBar";

// 2026-07-24 (declutter fix): the generic branch serves ALL non-news
// origins, not just cmux — `origin` is the five-value `SourceKind` union
// (`src/useSlotState.ts`'s `SOURCE_KINDS`). An exhaustive per-origin
// lookup (not a `=== "cmux"` ternary) keeps a future SourceKind addition
// a compile error here until this table is updated, same discipline as
// lib/presentation.ts's own exhaustive tables. "manual" is the `/notify`
// CLI push path, hence "cli"; "news" never reaches this branch (the
// `news` boolean above routes it to the news branch instead), so its
// entry is a defensive fallback, never actually read.
// M13 (layout overflow in the fixed 500x300 window): `liveVisibleDetails`
// used to render every pair the payload carried, uncapped — a worst-case
// server push (up to ~8 detail pairs, per the wire contract's own
// generous allowance) could grow `.compact` tall enough to push the
// TTL bar's floor strip out of the window (`.below-block`'s `overflow:
// hidden` just hard-crops instead of scrolling). Paired with
// `.detail-value`'s own 2-line clamp (manifest.css), this cap bounds the
// worst case to a knowable height instead of an unbounded one. Chosen
// well above every existing fixture's real usage (3 pairs, the richest
// StatusRailCard.test.tsx case) so no legitimate payload is ever visibly
// truncated in practice.
const MAX_VISIBLE_DETAIL_PAIRS = 4;

const GENERIC_MASTHEAD_KICKER: Record<SourceKind, string> = {
  cmux: "cmux",
  manual: "cli",
  football: "football",
  weather: "weather",
  news: "news",
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
            // `chip-cmux` "Agent" chip are gone (the kicker already says
            // cmux for that origin, so the chip was pure duplication).
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
              {/* an empty body (e.g. a cmux push with no body text)
                must not leave a blank `.notif-body` node in the card. */}
              {slot.body.trim() !== "" && <div className="notif-body">{bodyContent}</div>}
              {liveVisibleDetails.length > 0 &&
                liveVisibleDetails.slice(0, MAX_VISIBLE_DETAIL_PAIRS).map((detail) => (
                  <div key={`${detail.label}:${detail.value}`}>
                    <div className="detail-label">{detail.label}</div>
                    <div className="detail-value">{detail.value}</div>
                  </div>
                ))}
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
