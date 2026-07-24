import { motion } from "motion/react";
import { useEffect, useState } from "react";
import brandMark from "../../../assets/branding/notchtap-mark-128.png";
import { NOTCHTAP_EASE } from "../../animationTiming";
import { ActionStatus, useActionStatus } from "../actionStatus";
import { formatBytePair, formatBytes, formatUptime } from "../byteFormat";
import { SettingsGroup } from "../controls/controls";
import { settingsInvoke } from "../ipc";
import type { AboutInfo } from "../types";

const TECH_STACK = ["Rust core", "Tauri v2", "React + TypeScript", "Motion", "Vite"];

// Inline mono snippet, shared by every "How to use it" row — a smaller,
// less boxy sibling of DiagnosticsSection's <pre> log viewer (this is a
// few words inline in a sentence, not a multi-line block).
function Snippet({ children }: { children: string }) {
  return (
    <code className="rounded-[4px] border border-border/70 bg-input/30 px-[5px] py-[1px] font-mono text-fs-caption text-foreground">
      {children}
    </code>
  );
}

// About section: a rare-view tab earns a little delight (Emil Kowalski
// school — restraint everywhere else, animate what's rarely seen).
// Enters once on mount with a stagger; the 2s live-stat refresh
// below never remounts this array (same `info`-driven grid, values
// swapped in place), so nothing replays or moves on a poll tick — only
// the numbers change.
function StatTile({ label, value, index }: { label: string; value: string; index: number }) {
  return (
    <motion.div
      className="about-stat-tile rounded-md border border-border/60 bg-background/40 px-3 py-2.5"
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.2, ease: NOTCHTAP_EASE, delay: index * 0.04 }}
    >
      <div className="about-stat-label font-mono text-fs-caption font-bold tracking-[0.1em] text-muted-foreground uppercase">
        {label}
      </div>
      <div className="about-stat-value mt-1 font-mono text-fs-body text-foreground tabular-nums">
        {value}
      </div>
    </motion.div>
  );
}

// Fetch-on-open + a 2s live poll while the section stays mounted (unlike
// Diagnostics/Queue's manual-Refresh-only shape) — memory/disk are the
// one part of this section meant to visibly move. The interval is torn
// down on unmount, same pattern as the connector-health poll in
// SettingsApp.tsx, so switching away from About stops the polling
// entirely rather than leaking a background timer.
export function AboutSection() {
  const [info, setInfo] = useState<AboutInfo | null>(null);
  const { status, run } = useActionStatus("about");

  function refresh(announce: boolean) {
    void run(() => settingsInvoke("get_about_info").then((fetched) => setInfo(fetched)), {
      announce,
      showPending: false,
      errorMessage: () => "Couldn't read system info",
    });
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-only fetch + poll setup — refresh is re-created every render, so adding it would tear down and restart the interval on every render.
  useEffect(() => {
    refresh(false);
    const interval = setInterval(() => refresh(false), 2000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="section-stack">
      <SettingsGroup title="notchtap">
        <div className="about-identity flex items-center gap-3 py-2">
          <img
            src={brandMark}
            alt=""
            aria-hidden="true"
            className="about-mark h-10 w-10 flex-none"
          />
          <div className="min-w-0">
            <div className="text-fs-title leading-[1.2] font-[650] text-foreground">notchtap</div>
            <p className="mt-0.5 text-fs-body leading-[1.4] text-muted-foreground">
              Local-first notification HUD for your Mac.
            </p>
            <div className="about-version mt-1 font-mono text-fs-caption text-muted-foreground">
              {info ? `v${info.version} · ${info.bundleId}` : "Loading…"}
            </div>
          </div>
        </div>
      </SettingsGroup>

      <SettingsGroup title="What it is">
        <p className="m-0 py-2 text-fs-body leading-[1.5] text-muted-foreground">
          Cards from your tools slide out beside the clock — over the notch on a MacBook, or as a
          floating top-center HUD on a notchless display. Everything runs locally: a small Rust core
          listens on 127.0.0.1:9789, and the window only renders what the core sends it.
        </p>
      </SettingsGroup>

      <SettingsGroup title="How to use it">
        <ul className="how-to-use-list m-0 flex list-none flex-col gap-2.5 py-2 pl-0 text-fs-body leading-[1.5] text-muted-foreground">
          <li>
            <Snippet>{'./notchtap --title "t" --body "b"'}</Snippet> pushes a card.
          </li>
          <li>
            <Snippet>{"notchtap run -- <cmd>"}</Snippet> wraps a long-running command and pushes a
            completion card when it finishes.
          </li>
          <li>
            Connectors for football scores, personalized news topics, weather, and the cmux agent
            relay are configured in their own tabs.
          </li>
          <li>
            <Snippet>⌃⇧N</Snippet> expands a card, <Snippet>⌃⇧O</Snippet> opens its link,{" "}
            <Snippet>⌃⇧,</Snippet> opens settings.
          </li>
        </ul>
      </SettingsGroup>

      <SettingsGroup title="Tech stack">
        <div className="tech-stack-chips flex flex-wrap gap-1.5 py-2">
          {TECH_STACK.map((item) => (
            <span
              key={item}
              className="rounded-full border border-border px-[9px] py-[3px] font-mono text-fs-caption text-muted-foreground"
            >
              {item}
            </span>
          ))}
        </div>
        <p className="m-0 pb-1 text-fs-caption text-muted-foreground">
          Built across 130+ small reviewed plans.
        </p>
      </SettingsGroup>

      <SettingsGroup title="System" description="Live process, memory, and disk stats.">
        <ActionStatus status={status} className="about-status" showPending={false} />
        {info ? (
          <div className="about-stat-grid grid grid-cols-2 gap-2 py-2">
            <StatTile index={0} label="App memory" value={formatBytes(info.processMemoryBytes)} />
            <StatTile
              index={1}
              label="System memory"
              value={formatBytePair(info.systemMemoryUsedBytes, info.systemMemoryTotalBytes)}
            />
            <StatTile
              index={2}
              label="Disk"
              value={formatBytePair(info.diskUsedBytes, info.diskTotalBytes)}
            />
            <StatTile index={3} label="Platform" value={`${info.platform} · ${info.arch}`} />
            <StatTile index={4} label="Bundle size" value={formatBytes(info.bundleSizeBytes)} />
            <StatTile index={5} label="Uptime" value={formatUptime(info.uptimeSecs)} />
          </div>
        ) : status.state === "error" ? (
          <p className="about-empty m-0 py-3 text-fs-body text-muted-foreground">
            Couldn't load system info — it will retry automatically.
          </p>
        ) : (
          <p className="about-empty m-0 py-3 text-fs-body text-muted-foreground">Loading…</p>
        )}
      </SettingsGroup>
    </div>
  );
}
