import { AnimatePresence, motion } from "motion/react";
import { sourceLabelFor, type EventType } from "../lib/presentation";

// The hardcoded "⌃⇧N" hint mirrors EXPAND_TOGGLE_SHORTCUT in lib.rs (a
// hardcoded rust constant itself, since v3.6 spec §7.1 explicitly defers
// the exact combo) — restated here rather than threaded through the wire,
// since both sides are already hardcoded placeholders in lockstep.
export function Manifest({
  body,
  eventType,
  expanded,
}: {
  body: string;
  eventType: EventType;
  expanded: boolean;
}) {
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
          <div className="manifest-inner">
            <div>
              <div className="detail-label">Message</div>
              <div className="detail-value">{body}</div>
            </div>
            <div>
              <div className="detail-label">Source / control</div>
              <div className="detail-value">
                {sourceLabelFor(eventType)}
                <br />
                ⌃⇧N collapse
              </div>
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
