import { useMemo } from "react";
import { renderInlineMarkdown } from "../lib/markdown";
import type { EventType } from "../lib/presentation";

// The hardcoded "⌃⇧N" hint mirrors EXPAND_TOGGLE_SHORTCUT in lib.rs (a
// hardcoded rust constant itself, since v3.6 spec §7.1 explicitly defers
// the exact combo) — restated here rather than threaded through the wire,
// since both sides are already hardcoded placeholders in lockstep.
//
// 2026-07-24 (compact/expanded declutter): the expanded panel's ONLY job
// now is the summary/message rendering — label + body + the keyboard
// hint footer. Everything else that used to live here (news's
// `.manifest-meta` source/published/category footer; the generic
// branch's `.manifest-meta` source label and its `.manifest-fields`
// subtitle/detail cells) is compact-card metadata now — NotificationBody
// already surfaces all of it (masthead/meta row for news, masthead +
// subtitle row + detail cells for generic), so repeating it here was
// pure duplication. Both branches converge on the exact same
// `.manifest-block` shape below; only the label text and the summary
// source differ.
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

  // Change C: the rust side now sends an empty `body` for a redundant
  // Google-News summary (one that adds nothing beyond the headline) —
  // fall back to the full, untruncated title as the summary text rather
  // than showing an empty panel. Plain text either way, same as news's
  // summary always was (never markdown — that's the generic branch's
  // `messageContent` below).
  const newsSummary = body.trim() !== "" ? body : title;

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
