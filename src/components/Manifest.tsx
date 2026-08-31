import { useMemo } from "react";
import { renderInlineMarkdown } from "../lib/markdown";
import type { EventType } from "../lib/presentation";

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

  const newsSummary = body.trim() !== "" ? body : title;

  const messageContent = useMemo(() => renderInlineMarkdown(body), [body]);

  return (
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
