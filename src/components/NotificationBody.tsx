import type { ReactNode } from "react";
import type { SlotState } from "../useSlotState";
import { Manifest } from "./Manifest";
import { Stamp } from "./Stamp";
import type { Detail } from "./StatusRailCard";
import { Track } from "./Track";
import { TtlBar } from "./TtlBar";

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
            // plan 092 (item 19, this plan's core): the general
            // card's header row (title + the badge cluster) +
            // subtitle row (plan 035's `subtitle`, surfaced in
            // compact for the first time) + full-width clamped
            // body. There is no inline-time value here (no
            // non-news event carries a publishedAtMs), so the
            // subtitle row's time slot simply never renders.
            // plan 096: the badge cluster is the priority Stamp
            // PLUS the cmux chip, conditional on `origin` (now on
            // the wire — 092 deferred this exact spot pending
            // that wire change).
            <>
              <div className="notif-header-row">
                <span className="notif-title">{slot.title}</span>
                <div className="notif-header-badges">
                  {slot.origin === "cmux" && <span className="chip chip-cmux">Agent</span>}
                  <Stamp priority={slot.priority} signal={slot.signal} eventType={slot.eventType} />
                </div>
              </div>
              {slot.subtitle !== null && (
                <div className="notif-subtitle-row">
                  <span className="notif-subtitle">{slot.subtitle}</span>
                </div>
              )}
              <div className="notif-body">{bodyContent}</div>
              {/* plan 042: collapsed scorecard cells (Clock,
                per-side Cards) — only a live-match card with
                `espn_live_card` on populates `details`, so
                every other card renders exactly as before.
                Same detail-label/detail-value classes as the
                expanded Manifest view; collapsed-only, so the
                pairs never render twice when expanded. */}
              {!expanded &&
                liveVisibleDetails.length > 0 &&
                liveVisibleDetails.map((detail) => (
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
        <Track total={slot.queueTotal} done={slot.queueDone} />
      </div>
      <Manifest
        body={slot.body}
        eventType={slot.eventType}
        expanded={expanded}
        source={slot.source}
        category={slot.category}
        publishedAtMs={slot.publishedAtMs}
        hasLink={slot.link !== null}
        subtitle={slot.subtitle}
        details={liveVisibleDetails}
      />
      {/* plan 100: last in DOM order within .below-block — the bar
        is the card's floor, absolutely positioned to its bottom
        edge (styles.css), clipped to the rounded corners by
        .below-block's own overflow: hidden. */}
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
      />
    </>
  );
}
