import { type ReactNode, useId } from "react";

export type Tab = "agent" | "football" | "news";

export const TAB_ORDER: readonly Tab[] = ["agent", "football", "news"];

export type IconVisualState = "hidden" | "present" | "live";

export interface IconStripProps {
  agent: IconVisualState;
  football: IconVisualState;
  news: IconVisualState;
  newsCharge: number;
  newsCharged: boolean;
  newsCount: number | null;
  selected: Tab | null;
  onSelect?: (tab: Tab) => void;
}

const TAB_LABEL = {
  agent: "Agent",
  football: "Football",
  news: "News",
} satisfies Record<Tab, string>;

function iconAriaLabel(tab: Tab, newsCount: number | null): string {
  if (tab === "news" && newsCount !== null) {
    return `${TAB_LABEL[tab]}, ${newsCount} new`;
  }
  return TAB_LABEL[tab];
}

function AgentGlyph(): ReactNode {
  return (
    <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <path
        d="M4 5 L9 9 L4 13"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path d="M10.5 13 H14" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}

export function FootballGlyph(): ReactNode {
  return (
    <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <circle cx="9" cy="9" r="6.5" stroke="currentColor" strokeWidth="1.4" />
      <path
        d="M9 9 L9 3.5 M9 9 L13.7 11.7 M9 9 L4.3 11.7"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
    </svg>
  );
}

export function NewsGlyph({ charge }: { charge: number }): ReactNode {
  const clamped = Math.max(0, Math.min(1, charge));
  const clipId = useId();
  return (
    <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <rect x="3" y="2.5" width="12" height="13" rx="1.5" stroke="currentColor" strokeWidth="1.2" />
      <clipPath id={clipId}>
        <rect x="3" y="2.5" width="12" height="13" rx="1.5" />
      </clipPath>
      <rect
        className="charge"
        x="3"
        y="2.5"
        width="12"
        height="13"
        clipPath={`url(#${clipId})`}
        fill="currentColor"
        opacity="0.55"
        style={{ transform: `scaleY(${clamped})`, transformOrigin: "9px 15.5px" }}
      />
      <path
        d="M5.5 6 H12.5 M5.5 8.6 H10"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
      />
    </svg>
  );
}

function iconClass(tab: Tab, state: IconVisualState, selected: boolean, charged: boolean): string {
  const classes = [`icon ${tab}`];
  if (state !== "hidden") classes.push("is-present");
  if (state === "live") classes.push("is-live");
  if (selected) classes.push("is-selected");
  if (tab === "news" && charged) classes.push("is-charged");
  return classes.join(" ");
}

export function IconStrip({
  agent,
  football,
  news,
  newsCharge,
  newsCharged,
  newsCount,
  selected,
  onSelect,
}: IconStripProps) {
  const states = { agent, football, news } satisfies Record<Tab, IconVisualState>;
  return (
    <span className="icon-strip">
      {TAB_ORDER.map((tab) => {
        const state = states[tab];
        const isPresent = state !== "hidden";
        const isSelected = selected === tab;
        const isCharged = tab === "news" && newsCharged;
        return (
          <button
            key={tab}
            type="button"
            className={iconClass(tab, state, isSelected, isCharged)}
            aria-label={iconAriaLabel(tab, newsCount)}
            aria-pressed={isSelected}
            disabled={!isPresent}
            onClick={isPresent ? () => onSelect?.(tab) : undefined}
          >
            {tab === "agent" && <AgentGlyph />}
            {tab === "football" && <FootballGlyph />}
            {tab === "news" && <NewsGlyph charge={newsCharge} />}
            {tab === "news" && newsCount !== null && (
              <span className="charge-count">{newsCount}</span>
            )}
          </button>
        );
      })}
    </span>
  );
}
