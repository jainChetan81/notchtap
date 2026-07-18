import { AnimatePresence, motion } from "motion/react";
import { renderInlineMarkdown } from "../lib/markdown";
import { categoryLabel, type EventType, publishedLabel, sourceLabelFor } from "../lib/presentation";

// The hardcoded "⌃⇧N" hint mirrors EXPAND_TOGGLE_SHORTCUT in lib.rs (a
// hardcoded rust constant itself, since v3.6 spec §7.1 explicitly defers
// the exact combo) — restated here rather than threaded through the wire,
// since both sides are already hardcoded placeholders in lockstep.
export function Manifest({
  body,
  eventType,
  expanded,
  source,
  category,
  publishedAtMs,
  hasLink,
  subtitle,
  details = [],
}: {
  body: string;
  eventType: EventType;
  expanded: boolean;
  source?: string | null;
  category?: string | null;
  publishedAtMs?: number | null;
  hasLink: boolean;
  // plan 035 (Layout A): subtitle renders as its own manifest cell, and
  // each detail pair as one more cell — plain text, never markdown (they
  // originate in untrusted hook input). Only the generic branch shows them.
  subtitle?: string | null;
  details?: { label: string; value: string }[];
}) {
  const newsPublished = publishedLabel(publishedAtMs ?? null, Date.now());
  const newsCategory = categoryLabel(category ?? null);

  return (
    <AnimatePresence initial={false}>
      {expanded && (
        <motion.div
          className="manifest"
          initial={{ height: 0, opacity: 0 }}
          animate={{ height: "auto", opacity: 1 }}
          exit={{ height: 0, opacity: 0 }}
          transition={{ duration: 0.24, ease: [0.22, 1, 0.36, 1] }}
        >
          {eventType === "news_item" ? (
            <div className="manifest-inner news">
              <div>
                <div className="detail-label">Summary</div>
                <div className="detail-value">{body}</div>
              </div>
              <div>
                <div className="detail-label">Source / Published</div>
                <div className="detail-value">
                  {source ?? "RSS"}
                  {newsPublished !== null && (
                    <>
                      <br />
                      {newsPublished}
                    </>
                  )}
                </div>
              </div>
              <div>
                <div className="detail-label">Category / Control</div>
                <div className="detail-value">
                  {newsCategory !== null && (
                    <>
                      {newsCategory}
                      <br />
                    </>
                  )}
                  {hasLink ? "⌃⇧O read · ⌃⇧N collapse" : "⌃⇧N collapse"}
                </div>
              </div>
            </div>
          ) : (
            <div className="manifest-inner">
              <div>
                <div className="detail-label">Message</div>
                <div className="detail-value message">{renderInlineMarkdown(body)}</div>
              </div>
              <div>
                <div className="detail-label">Source / Control</div>
                <div className="detail-value">
                  {sourceLabelFor(eventType)}
                  <br />
                  {hasLink ? "⌃⇧O read · ⌃⇧N collapse" : "⌃⇧N collapse"}
                </div>
              </div>
              {subtitle ? (
                <div>
                  <div className="detail-label">Subtitle</div>
                  <div className="detail-value">{subtitle}</div>
                </div>
              ) : null}
              {details.map((detail) => (
                <div key={`${detail.label}:${detail.value}`}>
                  <div className="detail-label">{detail.label}</div>
                  <div className="detail-value">{detail.value}</div>
                </div>
              ))}
            </div>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
