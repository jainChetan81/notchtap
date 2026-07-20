import { Fragment, useMemo } from "react";
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
  const metaSegments = [
    <b key="src">{source ?? "RSS"}</b>,
    ...(newsPublished !== null ? [<span key="pub">published {newsPublished}</span>] : []),
    ...(newsCategory !== null ? [<span key="cat">{newsCategory}</span>] : []),
  ];

  // plan 069 (folded into 078): memoized on `body` so unrelated re-renders
  // don't re-tokenize the markdown.
  const messageContent = useMemo(() => renderInlineMarkdown(body), [body]);

  return (
    // plan 078: expand/collapse is now a CSS grid-template-rows 0fr→1fr
    // transition (styles.css) — the content stays mounted at all times, so
    // collapsed content needs aria-hidden to stay out of the accessibility
    // tree (AnimatePresence used to remove it from the DOM entirely).
    <div className={`manifest-wrap${expanded ? " expanded" : ""}`} aria-hidden={!expanded}>
      <div className="manifest">
        {eventType === "news_item" ? (
          <div className="manifest-block">
            <div className="manifest-label">Summary</div>
            <div className="manifest-text">{body}</div>
            <div className="manifest-meta">
              {metaSegments.map((segment, index) => (
                <Fragment key={segment.key}>
                  {index > 0 && <span className="sep">·</span>}
                  {segment}
                </Fragment>
              ))}
            </div>
            <div className="manifest-footer">
              <span className="manifest-hint">
                {hasLink ? (
                  <>
                    <kbd>⌃⇧O</kbd> read · <kbd>⌃⇧N</kbd> collapse
                  </>
                ) : (
                  <>
                    <kbd>⌃⇧N</kbd> collapse
                  </>
                )}
              </span>
            </div>
          </div>
        ) : (
          <div className="manifest-inner">
            <div>
              <div className="detail-label">Message</div>
              <div className="detail-value message">{messageContent}</div>
            </div>
            <div>
              <div className="detail-label">Source / Control</div>
              <div className="detail-value">
                {sourceLabelFor(eventType)}
                <br />
                {hasLink ? (
                  <>
                    <kbd>⌃⇧O</kbd> read · <kbd>⌃⇧N</kbd> collapse
                  </>
                ) : (
                  <>
                    <kbd>⌃⇧N</kbd> collapse
                  </>
                )}
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
      </div>
    </div>
  );
}
