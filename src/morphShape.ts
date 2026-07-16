// notch-morph nudge (IMPLEMENTATION_PLAN.md §3.5): which shape a
// notification morphs into, keyed by event type. this is a new column on the
// same event-type-keyed table v2.3's animations use (not a separate lookup
// mechanism) — in notch mode the shape's own enter/exit animation supersedes
// the event type's plain entrance animation; hud mode is untouched since it
// never applies pill/grow.
export type MorphShape = "pill" | "grow";

export function getMorphShape(eventType: string): MorphShape {
  switch (eventType) {
    case "score_update":
    case "match_state":
      return "pill";
    default:
      return "grow";
  }
}
