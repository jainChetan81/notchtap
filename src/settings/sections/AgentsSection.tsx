import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { MetaChip } from "@/components/ui/meta-chip";
import { Switch } from "@/components/ui/switch";
import { SOURCE_RUNTIME_COLORS } from "@/lib/sourceColors";
import { cn } from "@/lib/utils";
import { ActionStatus, useActionStatus } from "../actionStatus";
import {
  CONTROL_ROW,
  ControlCopy,
  NumberControl,
  SettingsGroup,
  ToggleControl,
} from "../controls/controls";
import { Segmented } from "../controls/Segmented";
import { settingsInvoke } from "../ipc";
import type {
  AdapterAvailability,
  AdapterErrorCategory,
  AdapterHealthDto,
  AgentRuntimesConfig,
  AgentsConfig,
  AgentWireRuntime,
  Config,
} from "../types";
import { PRIORITY_SEGMENT_OPTIONS, PRIORITY_TONES } from "../types";

// --- adapter card static content (plan 143, spec §4.6/§8) --------------
//
// Sourced from the committed `adapters/*/README.md` setup snippets (and
// the OpenCode plugin's own header comment) — inlined as constants so
// the section works with no extra IPC round trip, per this ticket's
// "keep it simple and truthful" instruction. Each snippet is the EXACT
// text a user copies into the EXACT target file named alongside it;
// notchtap never writes these itself (spec §4.6: "v7 does not silently
// edit a user's global provider configuration").

type AdapterConfigKey = keyof AgentRuntimesConfig;

interface AdapterCardCopy {
  configKey: AdapterConfigKey;
  wireRuntime: AgentWireRuntime;
  label: string;
  targetFile: string;
  snippet: string;
  uninstall: string;
}

const CLAUDE_CODE_SNIPPET = `{
  "hooks": {
    "SessionStart": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }],
    "SessionEnd": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }],
    "PermissionRequest": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }],
    "Notification": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }],
    "StopFailure": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }],
    "PostToolUse": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }],
    "PostToolUseFailure": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }],
    "SubagentStart": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }],
    "SubagentStop": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }]
  }
}`;

const CODEX_SNIPPET = `{
  "hooks": {
    "SessionStart": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }],
    "SessionEnd": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }],
    "PermissionRequest": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }],
    "SubagentStart": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }],
    "SubagentStop": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }],
    "PreToolUse": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }],
    "PostToolUse": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook codex" }] }]
  }
}`;

const KIMI_SNIPPET = `[[hooks]]
event = "SessionStart"
command = "notchtap-agent hook kimi"

[[hooks]]
event = "SessionEnd"
command = "notchtap-agent hook kimi"

[[hooks]]
event = "PermissionRequest"
command = "notchtap-agent hook kimi"

[[hooks]]
event = "Notification"
command = "notchtap-agent hook kimi"

[[hooks]]
event = "Stop"
command = "notchtap-agent hook kimi"

[[hooks]]
event = "StopFailure"
command = "notchtap-agent hook kimi"

[[hooks]]
event = "PostToolUse"
command = "notchtap-agent hook kimi"

[[hooks]]
event = "PostToolUseFailure"
command = "notchtap-agent hook kimi"

[[hooks]]
event = "SubagentStart"
command = "notchtap-agent hook kimi"

[[hooks]]
event = "SubagentStop"
command = "notchtap-agent hook kimi"`;

const OPENCODE_SNIPPET = `// copy (or symlink) adapters/opencode/notchtap.ts from the notchtap
// repo into this project's .opencode/plugins/ directory — no build
// step, no extra dependency.`;

const ADAPTER_CARDS: readonly AdapterCardCopy[] = [
  {
    configKey: "claude_code",
    wireRuntime: "claude-code",
    label: "Claude Code",
    targetFile: "~/.claude/settings.json (or a project's .claude/settings.json)",
    snippet: CLAUDE_CODE_SNIPPET,
    uninstall: "Remove the ten hook entries above from settings.json, or delete the whole file.",
  },
  {
    configKey: "codex",
    wireRuntime: "codex",
    label: "Codex",
    targetFile: "~/.codex/hooks.json (or a project's .codex/hooks.json)",
    snippet: CODEX_SNIPPET,
    uninstall: "Remove the eight hook entries above from hooks.json, or delete the whole file.",
  },
  {
    configKey: "kimi",
    wireRuntime: "kimi",
    label: "Kimi",
    targetFile: "~/.kimi-code/config.toml",
    snippet: KIMI_SNIPPET,
    uninstall: "Remove the ten [[hooks]] tables above from config.toml.",
  },
  {
    configKey: "opencode",
    wireRuntime: "opencode",
    label: "OpenCode",
    targetFile: ".opencode/plugins/notchtap.ts (project) or ~/.config/opencode/plugins/ (global)",
    snippet: OPENCODE_SNIPPET,
    uninstall: "Delete notchtap.ts from the plugins directory it was copied into.",
  },
];

const AVAILABILITY_LABELS: Record<AdapterAvailability, string> = {
  available: "Available",
  partial: "Partial",
  unavailable: "Unavailable",
};

// tone redesign: a binary active/not chip couldn't say "partial" apart
// from "unavailable" — both just read as un-emphasized. Real tri-state
// color so a glance at the dot tells you which of the three it is,
// without reading the word.
const AVAILABILITY_TONE: Record<AdapterAvailability, "positive" | "caution" | "critical"> = {
  available: "positive",
  partial: "caution",
  unavailable: "critical",
};

const ERROR_CATEGORY_LABELS: Record<AdapterErrorCategory, string> = {
  malformed_payload: "Malformed payload",
  unsupported_runtime: "Unsupported runtime",
  internal: "Internal error",
};

function formatLastSeen(ms: number | null): string {
  if (ms === null) return "Never";
  const elapsed = Date.now() - ms;
  if (elapsed < 0) return "Just now";
  const minutes = Math.floor(elapsed / 60000);
  if (minutes < 1) return "Just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  return `${Math.floor(hours / 24)} d ago`;
}

function patchAgents(
  config: Config,
  patchConfig: (patch: Partial<Config>) => void,
  patch: Partial<AgentsConfig>,
) {
  patchConfig({ agents: { ...config.agents, ...patch } });
}

function patchRuntime(
  config: Config,
  patchConfig: (patch: Partial<Config>) => void,
  key: AdapterConfigKey,
  enabled: boolean,
) {
  patchConfig({
    agents: {
      ...config.agents,
      runtimes: { ...config.agents.runtimes, [key]: { enabled } },
    },
  });
}

// Shared label/value field shape for the expanded panel's real facts —
// mirrors HistoryRow's `history-detail-field` stacked pair
// (HistorySection.tsx) so the two settings-window disclosure surfaces
// read as one visual language rather than two competing ones.
const AGENT_DETAIL_LABEL_CLASS =
  "agent-detail-label text-fs-caption tracking-[0.04em] text-muted-foreground uppercase";
const AGENT_DETAIL_VALUE_CLASS =
  "agent-detail-value min-w-0 text-fs-body text-muted-foreground [overflow-wrap:anywhere]";

function AdapterCard({
  copy,
  config,
  patchConfig,
  health,
}: {
  copy: AdapterCardCopy;
  config: Config;
  patchConfig: (patch: Partial<Config>) => void;
  health: AdapterHealthDto | undefined;
}) {
  // Handy "Models"-page reference: compact by default, click to reveal
  // real detail — no per-card lift needed, nothing outside this card
  // ever reads another card's expanded state.
  const [expanded, setExpanded] = useState(false);
  const { status: copyStatus, run: runCopy } = useActionStatus(`agent-copy-${copy.configKey}`);
  const { status: testStatus, run: runTest } = useActionStatus(`agent-test-${copy.configKey}`);
  const enabled = config.agents.runtimes[copy.configKey].enabled;
  const toggleId = `agent-runtime-${copy.configKey}`;
  const detailId = `agent-detail-${copy.configKey}`;

  async function copySnippet() {
    await runCopy(
      async () => {
        await navigator.clipboard.writeText(copy.snippet);
      },
      { announce: true, okMessage: "Copied", errorMessage: () => "Couldn't copy to clipboard" },
    );
  }

  async function sendTest() {
    await runTest(() => settingsInvoke("send_agent_test_event", { runtime: copy.wireRuntime }), {
      announce: true,
      okMessage: "Sent",
      errorMessage: (reason) =>
        typeof reason === "string" ? reason : "couldn't send a test event",
    });
  }

  return (
    <div className="agent-card border-t border-border/60 py-3 first:border-t-0">
      <div className="agent-card-header flex items-center justify-between gap-2">
        {/* Disclosure trigger: identity dot + name ONLY. The health chip
            and the enable switch below are siblings, not children, of
            this button — deliberately, so they stay visible and
            independently clickable while the card is collapsed (the
            switch is a primary control, not detail, per the ticket's
            Handy-modeled "Models" reference). A native <details> can't
            express that split (everything but <summary> hides when
            closed), so this is a plain controlled disclosure
            (aria-expanded/aria-controls) instead of this file's sibling
            HistorySection.tsx convention. */}
        <button
          type="button"
          className="agent-card-trigger flex min-w-0 flex-1 items-center gap-1.5 rounded-sm text-left outline-none transition-transform duration-[140ms] ease-notchtap focus-visible:ring-2 focus-visible:ring-ring active:scale-[0.97]"
          aria-expanded={expanded}
          aria-controls={detailId}
          onClick={() => setExpanded((prev) => !prev)}
        >
          <span
            data-slot="agent-runtime-dot"
            className="agent-runtime-dot inline-block size-[6px] flex-none rounded-full"
            style={{ background: SOURCE_RUNTIME_COLORS[copy.wireRuntime] }}
          />
          <span className="truncate text-fs-body font-[590] text-foreground">{copy.label}</span>
        </button>
        <div className="flex flex-none items-center gap-1.5">
          {health ? (
            <MetaChip uppercase tone={AVAILABILITY_TONE[health.status]}>
              {AVAILABILITY_LABELS[health.status]}
            </MetaChip>
          ) : null}
          {/* sr-only label + bare Switch (not the full ToggleControl row)
              — ToggleControl's own ControlCopy name/help pair is sized
              for a full-width settings row, not this compact header;
              the accessible name ("Enable {label}") is unchanged so the
              existing "enabled-runtime toggle round-trips" test still
              resolves it via screen.findByLabelText regardless of
              expanded state. */}
          <Label htmlFor={toggleId} className="sr-only">
            {`Enable ${copy.label}`}
          </Label>
          <Switch
            id={toggleId}
            checked={enabled}
            onCheckedChange={(next) => patchRuntime(config, patchConfig, copy.configKey, next)}
          />
        </div>
      </div>

      {expanded ? (
        // Mounted only while expanded (not a native <details>/CSS-hide):
        // keeps the setup snippet's Copy/Send-test buttons out of the
        // tab order entirely while collapsed, rather than fighting the
        // "interactive control nested in a hidden-but-still-focusable
        // subtree" trap a CSS-only hide would create. `animate-in
        // fade-in slide-in-from-top-1` (tw-animate-css, already imported
        // by base.css) plays reliably on insertion — a plain CSS
        // `transition` does not fire the same way across a display:none
        // boundary. duration/ease match this file's other motion
        // (Segmented.tsx's `duration-[140ms] ease-notchtap`).
        <div
          id={detailId}
          className="agent-card-detail mt-3 flex flex-col gap-3 animate-in fade-in slide-in-from-top-1 duration-[140ms] ease-notchtap"
        >
          <div className="agent-detail-field grid min-w-0 grid-cols-[minmax(0,1fr)] gap-px">
            <span className={AGENT_DETAIL_LABEL_CLASS}>Last seen</span>
            <span className={AGENT_DETAIL_VALUE_CLASS}>
              {formatLastSeen(health?.lastAcceptedEventMs ?? null)}
            </span>
          </div>
          {health?.lastErrorCategory ? (
            <div className="agent-detail-field grid min-w-0 grid-cols-[minmax(0,1fr)] gap-px">
              <span className={AGENT_DETAIL_LABEL_CLASS}>Last error</span>
              <span className={AGENT_DETAIL_VALUE_CLASS}>
                {ERROR_CATEGORY_LABELS[health.lastErrorCategory]}
              </span>
            </div>
          ) : null}
          {health?.compatibilityMessage ? (
            <div className="agent-detail-field grid min-w-0 grid-cols-[minmax(0,1fr)] gap-px">
              <span className={AGENT_DETAIL_LABEL_CLASS}>Status</span>
              <span className={AGENT_DETAIL_VALUE_CLASS}>{health.compatibilityMessage}</span>
            </div>
          ) : null}
          {health && health.capabilities.length > 0 ? (
            // Structured list, not a comma-joined string dressed as a
            // pill — each capability is its own line, no decorative chip
            // shape (the operator's explicit complaint about the old
            // "Capabilities: a, b, c" row).
            <div className="agent-detail-field grid min-w-0 grid-cols-[minmax(0,1fr)] gap-px">
              <span className={AGENT_DETAIL_LABEL_CLASS}>Capabilities</span>
              <ul
                className={cn(AGENT_DETAIL_VALUE_CLASS, "m-0 flex list-none flex-col gap-0.5 p-0")}
              >
                {health.capabilities.map((capability) => (
                  <li key={capability}>{capability}</li>
                ))}
              </ul>
            </div>
          ) : null}

          <div className="agent-setup">
            <div className="mb-1 text-fs-caption font-bold tracking-[0.06em] text-muted-foreground uppercase">
              Target file
            </div>
            <div className="mb-2 font-mono text-fs-secondary text-foreground [overflow-wrap:anywhere]">
              {copy.targetFile}
            </div>
            <pre className="agent-snippet m-0 max-h-[160px] overflow-auto rounded-sm border border-border bg-input/20 p-2 font-mono text-fs-caption whitespace-pre-wrap text-foreground [overflow-wrap:anywhere]">
              {copy.snippet}
            </pre>
          </div>

          <div className={CONTROL_ROW}>
            {/* `htmlFor` deliberately does not match any element id below —
                same convention `TestButtonRow` (controls.tsx) already uses:
                these two rows label a plain <Button> by its own visible
                text, not a form control a <label for> should actually
                associate with (associating would make the LABEL text win as
                the button's accessible name over its own text content). */}
            <ControlCopy
              htmlFor={`agent-copy-${copy.configKey}-label`}
              name="Setup snippet"
              help="Copy the block above into the target file shown."
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="text-fs-secondary"
              onClick={() => void copySnippet()}
            >
              Copy snippet
            </Button>
          </div>
          <ActionStatus status={copyStatus} className="agent-copy-status" />

          <div className={CONTROL_ROW}>
            <ControlCopy
              htmlFor={`agent-test-${copy.configKey}-label`}
              name="Test event"
              help="Post one synthetic completed event so you can see it land on the Agent Board."
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="text-fs-secondary"
              disabled={testStatus.state === "pending"}
              onClick={() => void sendTest()}
            >
              {testStatus.state === "pending" ? "Sending…" : "Send test event"}
            </Button>
          </div>
          <ActionStatus status={testStatus} className="agent-test-status" />

          <p className="agent-uninstall m-0 mt-1.5 text-fs-caption text-muted-foreground">
            Uninstall: {copy.uninstall}
          </p>
        </div>
      ) : null}
    </div>
  );
}

// Plan 143 (spec §8): static preview rows for the five Agent Board
// states the plan names — a simple, truthful text summary (runtime /
// state / summary), not a full card mockup. The Agent Board itself lives
// in the overlay (`App.tsx`), which the settings window never renders —
// see AppearanceSection's own preview-fixture doc for why the settings
// window's previews are always a lighter stand-in, never the live
// component.
const PREVIEW_FIXTURES: ReadonlyArray<{ label: string; runtime: string; summary: string }> = [
  {
    label: "Waiting on permission",
    runtime: "Claude Code",
    summary: "Approval needed to run a command",
  },
  {
    label: "Working, with a subagent",
    runtime: "Codex",
    summary: "Running tests — subagent: test runner (working)",
  },
  {
    label: "Completed",
    runtime: "Kimi",
    summary: "Turn completed — awaiting input",
  },
  {
    label: "Failed",
    runtime: "OpenCode",
    summary: "Session ended with an error",
  },
  {
    label: "Multiple independent sessions",
    runtime: "Claude Code + Codex",
    summary: "Two sessions active in different projects — each keeps its own history",
  },
];

export function AgentsSection({
  config,
  patchConfig,
}: {
  config: Config;
  patchConfig: (patch: Partial<Config>) => void;
}) {
  const [health, setHealth] = useState<AdapterHealthDto[] | null>(null);
  const { status: healthStatus, run: runHealthFetch } = useActionStatus("agent-health-load");

  function refreshHealth(announce: boolean) {
    void runHealthFetch(() => settingsInvoke("get_agent_health").then((rows) => setHealth(rows)), {
      announce,
      showPending: false,
      errorMessage: () => "Couldn't load adapter health",
    });
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-time fetch + a fixed poll interval — refreshHealth is re-created every render, so listing it would re-fire the effect every render.
  useEffect(() => {
    refreshHealth(false);
    const interval = setInterval(() => refreshHealth(false), 5000);
    return () => clearInterval(interval);
  }, []);

  function healthFor(configKey: AdapterConfigKey): AdapterHealthDto | undefined {
    const wireRuntime = ADAPTER_CARDS.find((c) => c.configKey === configKey)?.wireRuntime;
    return health?.find((h) => h.runtime === wireRuntime);
  }

  // Preview fixtures label a runtime by its display name (or, for the
  // "multiple sessions" row, a combo of names) rather than the wire
  // token — resolve back to an ADAPTER_CARDS entry when there's exactly
  // one match so the chip's dot stays in lockstep with the adapter
  // card's own colour; combo rows get no dot (no single colour is
  // honest there).
  function runtimeDotFor(runtimeLabel: string): string | undefined {
    const card = ADAPTER_CARDS.find((c) => c.label === runtimeLabel);
    return card ? SOURCE_RUNTIME_COLORS[card.wireRuntime] : undefined;
  }

  return (
    <div className="section-stack">
      <SettingsGroup
        title="Agent Adapters"
        description="Accept lifecycle events from coding-agent runtimes over the loopback /agent/events endpoint and show them on the Agent Board."
      >
        <ToggleControl
          id="agents-enabled"
          name="Enable Agent Adapters"
          help="Master switch — off skips every runtime's events regardless of the per-adapter toggles below."
          label="Enable Agent Adapters"
          checked={config.agents.enabled}
          onChange={(enabled) => patchAgents(config, patchConfig, { enabled })}
        />
        <NumberControl
          id="agents-terminal-retention"
          name="Terminal retention"
          help="How long a completed or failed session stays on the Agent Board before it's dropped."
          value={config.agents.terminal_retention_secs}
          min={0}
          max={86400}
          unit="SEC"
          onChange={(terminal_retention_secs) =>
            patchAgents(config, patchConfig, { terminal_retention_secs })
          }
        />
        <NumberControl
          id="agents-stale-after"
          name="Stale threshold"
          help="A session with no accepted event for this long is marked Stale on the Agent Board. Must be at least 1 second."
          value={config.agents.stale_after_secs}
          min={1}
          max={86400}
          unit="SEC"
          onChange={(stale_after_secs) => patchAgents(config, patchConfig, { stale_after_secs })}
        />
        <NumberControl
          id="agents-stale-retention"
          name="Stale retention"
          help="How long a Stale session stays on the Agent Board before it's dropped."
          value={config.agents.stale_retention_secs}
          min={0}
          max={86400}
          unit="SEC"
          onChange={(stale_retention_secs) =>
            patchAgents(config, patchConfig, { stale_retention_secs })
          }
        />
        <ToggleControl
          id="agents-completion"
          name="Completion cards"
          help="Runtimes fire a completion event after every response, not just at session end — turn this off to stop a card per turn."
          label="Show a card when an agent finishes a turn"
          checked={config.agents.completion_notifications}
          onChange={(completion_notifications) =>
            patchAgents(config, patchConfig, { completion_notifications })
          }
        />
        <ToggleControl
          id="agents-board-show-working"
          name="Board for working sessions"
          help="Off (the default) keeps the Agent Board out of the way until something needs you — a permission or input request, a failure, or a session that just finished. It still lists the working sessions once it's up."
          label="Show the Agent Board while agents are only working"
          checked={config.agents.board_show_working}
          onChange={(board_show_working) =>
            patchAgents(config, patchConfig, { board_show_working })
          }
        />
        <ToggleControl
          id="agents-informational"
          name="Informational cards"
          help="Also show a card for ordinary progress/tool events, not just permission/input/failure/completion."
          label="Show informational cards"
          checked={config.agents.informational_notifications}
          onChange={(informational_notifications) =>
            patchAgents(config, patchConfig, { informational_notifications })
          }
        />
      </SettingsGroup>

      <SettingsGroup
        title="Notification priority"
        description="Which rotation tier each kind of Agent event promotes in."
      >
        <Segmented
          id="agents-permission-priority"
          name="Permission requested"
          help="Priority for a permission-request event."
          options={PRIORITY_SEGMENT_OPTIONS}
          optionTones={PRIORITY_TONES}
          value={config.agents.permission_priority}
          onChange={(permission_priority) =>
            patchAgents(config, patchConfig, { permission_priority })
          }
        />
        <Segmented
          id="agents-input-priority"
          name="Input required"
          help="Priority for an explicit-input-required event."
          options={PRIORITY_SEGMENT_OPTIONS}
          optionTones={PRIORITY_TONES}
          value={config.agents.input_priority}
          onChange={(input_priority) => patchAgents(config, patchConfig, { input_priority })}
        />
        <Segmented
          id="agents-failure-priority"
          name="Failed"
          help="Priority for a terminal failure event."
          options={PRIORITY_SEGMENT_OPTIONS}
          optionTones={PRIORITY_TONES}
          value={config.agents.failure_priority}
          onChange={(failure_priority) => patchAgents(config, patchConfig, { failure_priority })}
        />
        <Segmented
          id="agents-completion-priority"
          name="Completed"
          help="Priority for a completion event (per-turn Stop or session end)."
          options={PRIORITY_SEGMENT_OPTIONS}
          optionTones={PRIORITY_TONES}
          value={config.agents.completion_priority}
          onChange={(completion_priority) =>
            patchAgents(config, patchConfig, { completion_priority })
          }
        />
      </SettingsGroup>

      <SettingsGroup
        title="Adapters"
        description="One card per supported runtime — setup snippet, declared capabilities, and live health."
      >
        <ActionStatus status={healthStatus} className="agent-health-status" showPending={false} />
        {ADAPTER_CARDS.map((copy) => (
          <AdapterCard
            key={copy.configKey}
            copy={copy}
            config={config}
            patchConfig={patchConfig}
            health={healthFor(copy.configKey)}
          />
        ))}
      </SettingsGroup>

      <SettingsGroup
        title="Preview"
        description="What each Agent Board state looks like — a text summary, not the live card (the Agent Board itself renders in the overlay)."
      >
        {PREVIEW_FIXTURES.map((sample) => (
          <div
            key={sample.label}
            className="agent-preview-row border-t border-border/60 py-2 first:border-t-0"
          >
            <div className="mb-0.5 flex items-center gap-1.5">
              <span className="text-fs-body font-[590] text-foreground">{sample.label}</span>
              <MetaChip dotColor={runtimeDotFor(sample.runtime)}>{sample.runtime}</MetaChip>
            </div>
            <p className="m-0 text-fs-secondary text-muted-foreground">{sample.summary}</p>
          </div>
        ))}
      </SettingsGroup>
    </div>
  );
}
