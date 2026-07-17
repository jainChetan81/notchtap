import Lottie from "lottie-react";
// Self-authored, bundled locally — never fetched from a CDN at runtime
// (this is an always-on background utility that must work offline).
// Original work, same license as the rest of this repo — no third-party
// asset license to track, unlike a sourced LottieFiles download would be.
import goalCelebration from "../assets/lottie/goal-celebration.json";

// Mounted by <StatusRailCard> only while signal === "goal" (never on
// priority alone — see that component's pulse-trigger effect). Plays
// once and never loops: a persistent overlay replaying a celebration
// animation indefinitely would be obnoxious, not celebratory.
export function GoalCelebration() {
  return (
    <div className="goal-celebration" aria-hidden="true">
      <Lottie animationData={goalCelebration} loop={false} autoplay />
    </div>
  );
}
