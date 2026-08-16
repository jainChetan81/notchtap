import { useMemo } from "react";
import { renderInlineMarkdown } from "../lib/markdown";
import type { EventType } from "../lib/presentation";

// The hardcoded "⌃⇧N" hint mirrors EXPAND_TOGGLE_SHORTCUT in lib.rs (a
// hardcoded rust constant in lockstep) — restated here rather than
// threaded through the wire.
//
// The expanded panel's ONLY job is the summary/message rendering —
// label + body + the keyboard hint footer. NotificationBody already
// surfaces all other metadata (masthead/meta row for news, masthead +
// subtitle row + detail cells for generic), so repeating it here would
// duplicate. Both branches converge on the exact same `.manifest-block`
// shape below; only the label text and the summary source differ.
export function Manifest({
  title,
  body,
  eventType,
  expanded,
  hasLink,
}: {
  title: string;
  body: string;
  eventType: EventType;
  expanded: boolean;
  hasLink: boolean;
}) {
  const isNews = eventType === "news_item";

  // Rust sends an empty `body` for a redundant Google-News summary (one
  // adding nothing beyond the headline) — fall back to the full,
  // untruncated title rather than showing an empty panel. Plain text
  // either way; news summary is never markdown (that's `messageContent`).
  const newsSummary = body.trim() !== "" ? body : title;

  // Memoized on `body` so unrelated re-renders don't re-tokenize markdown.
  const messageContent = useMemo(() => renderInlineMarkdown(body), [body]);

  return (
    // Expand/collapse is a CSS grid-template-rows 0fr→1fr transition
    // (styles.css) — content stays mounted, so collapsed content needs
    // aria-hidden to stay out of the accessibility tree.
    <div className={`manifest-wrap${expanded ? " expanded" : ""}`} aria-hidden={!expanded}>
      <div className="manifest">
        <div className="manifest-block">
          <div className="manifest-label">{isNews ? "Summary" : "Message"}</div>
          <div className="manifest-text">{isNews ? newsSummary : messageContent}</div>
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
      </div>
    </div>
  );
}
