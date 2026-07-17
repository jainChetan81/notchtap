import { SignalLow, SignalMedium, SignalHigh } from "lucide-react";
import { tierCode, tierLabel, type Priority } from "../lib/presentation";

// The Lucide icon is the review's requested non-color priority cue —
// deliberately a *second*, independent signal alongside the code text
// and the Track rail below, not a replacement for either (a 3px color
// strip alone was the review's exact complaint: "Low" was nearly
// invisible and easy to miss in peripheral vision).
const TIER_ICONS: Record<Priority, typeof SignalLow> = {
  low: SignalLow,
  medium: SignalMedium,
  high: SignalHigh,
};

export function TierCode({ priority }: { priority: Priority }) {
  const Icon = TIER_ICONS[priority];
  return (
    <div className="tier-code">
      <Icon size={12} aria-hidden="true" />
      <span className="code">{tierCode(priority)}</span>
      <small className="tier-label">{tierLabel(priority)}</small>
    </div>
  );
}
