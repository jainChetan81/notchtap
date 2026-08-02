# notchtap — v7 technical spec (v0 draft): agent integrations

operationalizes `ARCHITECTURE.md` §20 and
`IMPLEMENTATION_PLAN.md` §9 into code-level specifics. this is a
working draft: implementation details may move as the provider hooks
are exercised, but changes to defaults, scope, or the security model
belong in `ARCHITECTURE.md`.

the product decisions were locked in a grilling session on 2026-07-26.
the initial Agent Runtimes are Claude Code, Codex, Kimi, and OpenCode.

---

## 0. outcome and scope

v7 replaces the cmux-specific notification source with a provider-neutral
**Agent Adapter** layer and adds the **Agent Board**, a rich idle surface
for independent Agent Sessions.

**in v7:**

- hook/plugin adapters for Claude Code, Codex, Kimi, and OpenCode;
- a versioned `POST /agent/events` loopback endpoint;
- one normalized Agent event/session model and an authoritative Rust
  Agent Registry;
- noteworthy Agent Events entering the existing Notification Slot;
- continuously updated, independent session rows on the Agent Board;
- honest adapter capability reporting and per-runtime health in Settings;
- a host-dependent Open/Focus Session action;
- complete removal of cmux-specific active code, config, UI, hooks, and
  active documentation;
- config/history migration from the old `Cmux` Origin to `Agent`.

**not in v7:**

- launching, supervising, or scraping coding-agent processes;
- terminal-output parsing, PTY ownership, or T3 Code-specific APIs;
- approve/reject/reply controls in the overlay;
- merging histories because two sessions share a runtime or project;
- inventing unsupported provider data;
- a notchtap MCP server. MCP remains an optional future control-plane
  integration, not the lifecycle transport.

T3 Code works without a dedicated T3 adapter because it launches the
ordinary provider runtimes and their normal configuration. `AgentHost`
may report T3 Code for presentation/focus, but Host is not Origin,
Runtime, or Agent Session identity.

## 1. provider capability contract

adapters declare capabilities when they report a session. the UI renders
only declared and observed capabilities; a missing capability is not an
error and must never be filled by title/body heuristics.

| capability | Claude Code | Codex | Kimi | OpenCode |
|---|---:|---:|---:|---:|
| session start/end | yes | yes | yes | yes |
| permission requested | yes | yes | yes | yes |
| explicit input required | notification-derived | not documented | notification-derived | session/status-derived |
| completed/stop | yes | yes | yes | idle/session-derived |
| failed | stop/tool failure | not documented (no failure hook in current Codex docs — verified 2026-07-26) | stop/tool failure | session error |
| tool detail | yes | yes | yes | yes |
| subagent lifecycle | yes | yes | yes | not declared until verified |
| Open/Focus Session | Host-dependent | Host-dependent | Host-dependent | Host-dependent |

the matrix is a minimum truthfulness contract, not a permanent ranking.
each adapter has its own compatibility tests and may add a capability
after a documented provider event has been verified.

primary contracts consulted for this draft:

- [Claude Code hooks](https://code.claude.com/docs/en/hooks)
- [Codex hooks](https://developers.openai.com/codex/hooks)
- [Kimi Code hooks](https://moonshotai.github.io/kimi-code/en/customization/hooks)
- [OpenCode plugins](https://opencode.ai/docs/plugins/)

Kimi hook support is version-gated because its hook surface is newer and
has appeared under both Kimi CLI and Kimi Code documentation. the adapter
must report `unavailable` with the minimum supported version when hooks
are absent; it must not silently fall back to scraping.

## 2. normalized domain model

new Rust module: `src-tauri/src/agents/`.

```text
agents/
  mod.rs
  adapter.rs        provider-neutral wire parsing + limits
  model.rs          runtime, session, event, capability types
  registry.rs       authoritative session state + ordering + retention
  notification.rs   noteworthy-event → existing Event mapping
  health.rs         per-runtime health/capability snapshot
  focus.rs          validated Host focus/open behavior
```

conceptual types:

```rust
enum AgentRuntime {
    ClaudeCode,
    Codex,
    Kimi,
    OpenCode,
}

enum AgentCapability {
    SessionLifecycle,
    PermissionRequests,
    InputRequired,
    Completion,
    Failure,
    ToolDetails,
    Subagents,
    OpenOrFocus,
}

enum AgentEventKind {
    PermissionRequested,
    InputRequired,
    Completed,
    Failed,
    Informational,
}

enum AgentSessionState {
    Starting,
    Working,
    WaitingForPermission,
    WaitingForInput,
    Completed,
    Failed,
    Stale,
}

struct AgentSessionKey {
    runtime: AgentRuntime,
    native_session_id: String,
}
```

`AgentSessionKey` is the sole registry identity. project path, project
name, Host, and display title are mutable metadata and never merge two
sessions. a provider without a native session ID may use an adapter-made
fallback containing a process identity plus start timestamp; project
path alone is forbidden.

`AgentSession` contains:

- key, state, capabilities;
- first-seen, state-entered, last-seen, and optional terminal timestamps;
- optional project metadata (`name`, sanitized `cwd`);
- optional Host metadata;
- latest sanitized summary and bounded detail cells;
- bounded independent transition history;
- optional subagent summary owned by that session;
- last accepted sequence, if the adapter can supply one.

the registry is in memory and owned behind the same application-state
boundary as the Engine, but it is not part of the Notification Queue.
an Agent Event may update the registry, create a Notification, do both,
or do neither when it is a duplicate/stale event.

### 2.1 state transition rules

- `SessionStart` → `Starting`, then a work/tool event → `Working`.
- permission event → `WaitingForPermission`.
- explicit idle/input event → `WaitingForInput`.
- a subsequent tool/work event clears either waiting state → `Working`.
- normal terminal event → `Completed`.
- a completed event that is NOT terminal (a per-turn provider `Stop`,
  OpenCode `session.idle`) → the session stays live in
  `WaitingForInput`: the turn finished and the agent awaits the user.
  only an explicit session-end event (`SessionEnd`,
  `session.deleted`) is terminal. (operator decision 2026-07-26 —
  per-turn Stop must not fragment one session into suffixed terminal
  rows.) the same non-terminal completed event is also a QUIET
  informational Notification, not a session-completed card — see §5's
  `Completed` split (operator decision 2026-08-02).
- failure event representing a terminal failure → `Failed`.
- a non-terminal tool failure is an informational/failure Notification
  while the session remains `Working`.
- a non-terminal session with no accepted event for
  `agents.stale_after_secs` → `Stale`.
- terminal sessions remain on the board for
  `agents.terminal_retention_secs`, default 60 (originally 600; lowered
  2026-07-27 by operator decision — a ten-minute completed row reads as
  a stuck notification), then leave the live registry view.
- waiting sessions do not rotate out or expire like Notifications.
  they can become Stale only through the explicit stale threshold.

terminal states never transition back to active for the same key. if a
provider incorrectly reuses a terminal ID, the adapter must suffix its
fallback generation rather than mutate old history.

### 2.2 ordering

the Agent Board ordering key is:

1. state urgency:
   `WaitingForPermission`, `WaitingForInput`, `Failed`, `Stale`,
   `Working`, `Starting`, `Completed`;
2. state-entered timestamp, oldest first;
3. first-seen timestamp, oldest first;
4. stable `AgentSessionKey` lexical tie-break.

this implements urgency first and FIFO within equal urgency without
render-time instability. a state change creates a new FIFO position in
the destination urgency class.

### 2.3 semantic equality

`AgentState::dedup_eq` must be handwritten. continuously varying fields
such as wall-clock-derived elapsed time, retention remaining, and
`last_seen_at_ms` do not count as content changes. state, summary,
capabilities, ordering, project/Host display metadata, and transition
history do.

never derive `PartialEq` and use it for publish suppression on an Agent
wire snapshot. this is the same invariant as `SlotState::dedup_eq`.

## 3. `POST /agent/events`

the provider hooks do not post to `/notify`. Agent Session updates are
not Notifications, and overloading the old schema would throw away the
state/identity contract.

the endpoint reuses `/notify`'s listener, loopback binding, Host-header
defense, body limit posture, logging, and application lifecycle.

### 3.1 schema v1

```json
{
  "schemaVersion": 1,
  "eventId": "runtime-generated-id",
  "runtime": "codex",
  "sessionId": "native-session-id",
  "occurredAtMs": 1785067200000,
  "sequence": 12,
  "nativeEvent": "PermissionRequest",
  "kind": "permission_requested",
  "state": "waiting_for_permission",
  "summary": "Approval needed to run a command",
  "details": [
    { "label": "Tool", "value": "shell" },
    { "label": "Project", "value": "notchtap" }
  ],
  "capabilities": [
    "session_lifecycle",
    "permission_requests",
    "tool_details"
  ],
  "project": {
    "name": "notchtap",
    "cwd": "/Users/example/code/notchtap"
  },
  "host": {
    "name": "T3 Code",
    "bundleId": "validated.adapter-owned.value"
  },
  "subagent": {
    "id": "native-subagent-id",
    "label": "test runner",
    "state": "working"
  },
  "terminal": false
}
```

`sequence`, project, Host, subagent, summary, and details are optional.
the adapter sends the normalized model, never a raw provider payload.

### 3.2 validation and bounds

- unknown `schemaVersion` → `400`;
- malformed JSON/enum or absent required identity → `400`;
- unsupported runtime → `400`;
- oversized body → `413`;
- accepted new state/event → `202`;
- duplicate `eventId` or stale sequence → idempotent `202`, with no
  registry/Notification change;
- internal registry/Engine failure → `500`.

hard caps are centralized in `agents/adapter.rs`:

| field | cap |
|---|---:|
| body | 64 KiB |
| event/session/native-event/Host IDs | 256 bytes each |
| summary | 500 Unicode scalar values |
| project name / Host name / labels | 120 Unicode scalar values |
| cwd / detail values | 1,024 Unicode scalar values |
| details | 12 |
| capabilities | 16 |
| subagents represented per event | 16 |
| retained transitions per session | 50 |
| remembered event IDs | 2,048, LRU |

strings are trimmed and control characters removed before storage or
rendering. secrets, prompts, raw tool input/output, environment values,
and complete command lines must not be forwarded. provider adapters may
extract a safe tool name, a basename, and a short human summary.

when `sequence` is present, a lower or equal value than the last accepted
sequence for that session is stale. without a sequence, receive order is
authoritative and `eventId` supplies duplicate protection. system clock
timestamps never override receive ordering.

## 4. adapter delivery

### 4.1 shared helper

add a small Rust binary target, `notchtap-agent`, installed with the app:

```text
notchtap-agent hook claude-code
notchtap-agent hook codex
notchtap-agent hook kimi
notchtap-agent test <runtime>
notchtap-agent status
notchtap-agent doctor
```

`doctor` is read-only: it inspects the four runtimes' hook config files
(`~/.claude/settings.json`, `~/.codex/hooks.json`,
`~/.kimi-code/config.toml`, `~/.config/opencode/plugins/notchtap.ts`) and
reports which expected hook events are wired and whether each hook's
command string resolves to an executable — it never creates, edits, or
repairs any of them.

`hook` reads one native JSON payload from stdin, normalizes it, posts
schema v1 to the configured loopback port, and exits. delivery rules:

- connect/read timeout at most 750 ms;
- fail open: provider sessions are never blocked by notchtap absence or
  malformed optional data;
- exit 0 for delivery failures after writing a bounded diagnostic to
  notchtap's adapter log, never stdout;
- no decision JSON, approval answer, or mutation of the native event;
- no daemon, background supervisor, shell interpolation, or `jq`
  dependency;
- `NOTCHTAP_PORT` remains the explicit port override.

provider parsers are separate pure functions with committed redacted
fixtures. native event names are recorded for diagnostics but mapping
branches only inside that provider's parser.

### 4.2 Claude Code

install command hooks for `SessionStart`, `SessionEnd`,
`PermissionRequest`, `Notification`, `Stop`, `StopFailure`,
`PostToolUse`, `PostToolUseFailure`, `SubagentStart`, and
`SubagentStop`.

`Notification` is accepted only for documented permission/idle input
notifications. generic notifications become `Informational`; wording is
not parsed to infer state.

### 4.3 Codex

install lifecycle hooks for documented events including `SessionStart`,
`SessionEnd`, `PermissionRequest`, `Stop`, `SubagentStart`,
`SubagentStop`, `PreToolUse`, `PostToolUse`, and failure variants
available in the installed Codex version.

Codex's legacy top-level `notify` command is not used. it is a single
user-global slot, may already be owned by another integration, and does
not expose the rich lifecycle contract. absence of an explicit
InputRequired or terminal-failure hook remains a declared capability
gap.

### 4.4 Kimi

install the equivalent Kimi hook events only when the local version
advertises hook support. Settings shows the detected compatibility
state and setup snippet. unsupported versions remain `unavailable`;
there is no terminal scraping fallback.

### 4.5 OpenCode

OpenCode uses a TypeScript plugin because its lifecycle surface is a
plugin event bus:

`adapters/opencode/notchtap.ts`.

the plugin listens for `permission.asked`, `permission.replied`,
`session.created`, `session.updated`, `session.status`, `session.idle`,
`session.error`, `session.deleted`, `tool.execute.before`, and
`tool.execute.after`, then posts the same normalized schema. its network
behavior, limits, sanitization, and fail-open semantics match the Rust
helper.

### 4.6 setup ownership

v7 does not silently edit a user's global provider configuration.
Settings provides:

- detected/undetected status;
- minimum compatible provider version when known;
- copyable setup snippets and exact target file;
- a test event;
- last-seen timestamp and declared capabilities;
- uninstall instructions.

automatic install/uninstall may be reconsidered only with explicit
backup, conflict detection, and preview UI.

## 5. registry → Notification mapping

all Agent events update the Agent Registry. only noteworthy kinds also
enter the existing Engine as `Origin::Agent`:

| kind | default Priority | Notification behavior |
|---|---|---|
| Permission Requested | High | one-shot |
| Input Required | High | one-shot |
| Failed (terminal) | High | one-shot |
| Completed (terminal) | Medium (`completion_priority`) | one-shot; on by default, suppressed by `completion_notifications = false` (§7) |
| Completed (non-terminal) | Medium (fixed) | off by default; follows the Informational row's gating |
| Informational | Medium | off by default; runtime/user policy may enable |

a NON-terminal tool failure follows the Informational row's gating
(off by default), not the Failed row — §2.1's "informational/failure
Notification" wording governs. operator-confirmed 2026-07-26.

**`Completed` carries the same terminal split** (operator decision
2026-08-02). every runtime fires a completion event twice-shaped: once
per response/turn (`Stop` / `session.idle`, `terminal: false`) and once
when the session genuinely ends (`SessionEnd` / `session.deleted`,
`terminal: true`). the per-turn stop is progress, not an outcome, so it
rides `informational_notifications` (off by default) at the fixed
Informational Medium — the operator is not carded once per turn. only
the terminal shape reads `completion_notifications` /
`completion_priority`. the two gates are independent: switching the
session-end card off does not silence a per-turn stop the operator
explicitly opted into, and vice versa. card text splits too — terminal
reads "X finished / Session completed.", non-terminal reads "X finished
a turn / Turn completed; the session is still open."

for this split to work, every runtime's REAL session end must arrive as
`Completed` + `terminal: true`. all four adapters do: Claude Code /
Codex / Kimi map their `SessionEnd` hook that way, and OpenCode's
`session.deleted` was corrected from `Informational` + `terminal: true`
(which produced no card at all, since it fell into the off-by-default
Informational gate) to match. `registry::next_state` resolves both
shapes to the same terminal `Completed` state, so the Agent Board is
unchanged by that correction.

Starting, Working, tool progress, and subagent progress update the Agent
Board without creating cards. Notification creation does not delete or
replace session history.

the generated existing-domain `Event` uses `EventType::AgentEvent` and
an `AgentSignal` carrying runtime, session key, kind, and sanitized
summary. `EventMeta.agent` contains presentation metadata. the Origin is
always `Agent`, regardless of runtime or Host.

all noteworthy events still obey the existing Queue/Slot rules:
Priority, Rotation Order, tier caps, Paused, Promotion, and Connectors.
Agent Registry updates are accepted even if the corresponding
Notification is rejected because its queue tier is full. the endpoint
returns `202` with a diagnostic `notificationQueued: false`; losing an
ephemeral card must not lose authoritative session state.

## 6. Agent Board IPC and presentation

Rust publishes `agent-state` independently of `slot-state` and
`status-state`.

```ts
type AgentState = {
  revision: number
  sessions: AgentSessionView[] // already ordered by Rust
  adapterHealth: AdapterHealthView[]
}
```

the overlay adds `useAgentState` and renders the result; it performs no
sorting, lifecycle inference, expiry, or history merging.

### 6.1 presentation precedence

1. a Visible Notification owns the Slot;
2. when the Slot is empty and the published `agent-state` snapshot holds
   at least one Agent Session, show the Agent Board;
3. otherwise show the existing clock/weather/media idle presentation.

when a noteworthy Agent Notification finishes, presentation returns to
the still-current Agent Board.

**board presence gate** (operator decision 2026-08-02): the Board's job
is ATTENTION, so a session that is merely running does not summon it. a
live/retained session existing is therefore no longer sufficient for
rule 2 — `[agents] board_show_working` (default `false`) decides:

- `false` (default): the Board is present only while at least one
  session is in an ATTENTION state — `waiting_for_permission`,
  `waiting_for_input`, `failed`, or a `completed` session still inside
  its `terminal_retention_secs` window. `working`, `starting`, and
  `stale` sessions alone leave the notch on its ordinary idle face.
- `true`: the pre-2026-08-02 behaviour — any live/retained session shows
  the Board.

this gates PRESENCE, never CONTENT. once any session has summoned the
Board, it lists every retained session in the usual §2.2 order, working
ones included.

the gate lives in exactly ONE place: `AgentBoardPublisher::
gate_presence` (`src-tauri/src/agents/board.rs`), applied to
`ordered_states` BEFORE the §2.3 dedup comparison. a gated-off Board
publishes as zero sessions, so every downstream consumer follows without
a second gate of its own — the overlay's `presentationMode`
(`src/lib/presentation.ts`) falls through to idle, and Rust's own
hover-expand (`last_session_count`, `try_expand_board_for_hover`) reads
`0` and declines to expand an undisplayed Board.

the default is a deliberate BEHAVIOUR CHANGE for existing installs: a
`config.toml` predating the key loads as `false` and stops summoning the
Board for working-only sessions. quiet-by-default is the point.

### 6.2 resting and expanded states

resting:

- one rich card for the highest-ranked session;
- every other session represented individually, never collapsed into
  a `+N` count;
- runtime, state, project, elapsed state time, and most recent safe
  summary;
- strong but non-alarming distinctions between waiting, failure,
  working, and completed.

hover/expanded:

- every retained session in Rust-provided order;
- screen-bounded maximum height and scrolling;
- each row expands to its independent bounded transition history;
- capability-dependent detail cells are omitted cleanly. (no
  reduced-motion mode — a standing repo non-goal.)

the existing native tracking area remains the source of hover truth.
on entry, Rust switches both the tracking rect and visual geometry to
the expanded Board bounds and temporarily enables pointer delivery
inside that exact rect so wheel scrolling works. on exit/collapse it
restores `ignoresMouseEvents = true` immediately. this requires manual
AppKit verification because the panel overlaps the menu-bar level.

the overlay remains receive-only. it listens to state and hover events
and sends no invoke/event command to Rust. therefore v7's initial
Open/Focus action is the global shortcut `⌃⇧A`, which focuses the
highest-ranked Agent Session. exact per-row click actions are deferred
unless they can preserve the receive-only boundary.

### 6.3 Open/Focus Session security

Host metadata is advisory. adapters cannot provide arbitrary commands
or URLs to execute.

- supported Host bundle IDs and activation strategies are owned by
  notchtap code, keyed by a small enum;
- unknown Host metadata renders as text but is not actionable;
- focus first tries the known Host application;
- an optional provider-native deep link is allowed only from a
  code-owned scheme allowlist and must match the session's provider;
- no `sh -c`, arbitrary executable path, or adapter-provided arguments;
- failure is logged and surfaced as a quiet status, never converted to
  shell fallback.

## 7. configuration and migration

new config block:

```toml
[agents]
enabled = true
terminal_retention_secs = 60
stale_after_secs = 300
stale_retention_secs = 600
informational_notifications = false
completion_notifications = true
board_show_working = false
permission_priority = "high"
input_priority = "high"
failure_priority = "high"
completion_priority = "medium"

[agents.runtimes.claude_code]
enabled = true

[agents.runtimes.codex]
enabled = true

[agents.runtimes.kimi]
enabled = true

[agents.runtimes.opencode]
enabled = true
```

`completion_notifications` (added 2026-08-02, operator decision) gates a
TERMINAL `Completed` — a real session end — only. it defaults `true`,
and a config predating the key loads as `true`, so session ends card out
of the box. a NON-terminal `Completed` (the per-turn stop) is not
covered by this key at all; it rides `informational_notifications`
(default `false`), which is what keeps per-turn cards quiet by default.
see §5's `Completed` split.

`board_show_working` (added 2026-08-02, operator decision) gates the
Agent Board's PRESENCE, not any card — see §6.1's board presence gate
for the full rule. it defaults `false`, and unlike
`completion_notifications` above, a config predating the key ALSO loads
as `false`, deliberately changing behaviour for existing installs.

the existing source-level config gains `agent_ttl_secs` and
`agent_priority` only where the flat compatibility layer still requires
them. event-kind priorities from `[agents]` take precedence for
adapter-generated notifications.

migration is automatic and idempotent:

- `SourceKind` removes `Cmux` and adds `Agent`;
- deserialization accepts legacy `"cmux"` as `Agent` for one release;
- `cmux_priority` aliases to `agent_priority` only when the new key is
  absent;
- `cmux_ttl_secs` aliases to `agent_ttl_secs` only when the new key is
  absent;
- a rotation-order `Cmux` entry is rewritten in place to `Agent`, then
  the existing heal/dedupe behavior runs;
- persisted history with Origin `Cmux` reads as `Agent`;
- Settings and config serialization write only new names;
- the `notchtap` CLI removes cmux environment autodetection and
  `--source cmux`; ordinary manual pushes remain Origin `Manual`.

default Rotation Order becomes:

```text
[Football, Manual, Weather, Agent, News]
```

## 8. Settings

remove `CmuxSection` and replace it with an **Agents** section:

- global enable, retention, stale threshold, informational-card toggle;
- event-kind Priority and Rotation controls;
- four adapter cards with enabled state, health, last seen, capabilities,
  compatibility note, setup/copy action, and test event;
- preview fixtures for waiting permission, working with subagents,
  completed, failed, and multiple independent sessions;
- Agent in General's Rotation Order editor;
- Agent labels/filters in History and queue inspection;
- shortcut cheat-sheet entry for `⌃⇧A`.

no new overlay command is introduced. any new Settings command must be
added to `build.rs`, `capabilities/settings.json`, the settings-window
label guard, and the command ACL tests together.

## 9. persistence and privacy

live registry state is in memory. when existing `history_enabled` is
false, terminal sessions vanish after retention and are not written.
when enabled, persist only the normalized/sanitized session summary and
bounded transitions through the existing history ownership boundary.

never persist:

- raw native hook JSON;
- prompts or model responses;
- tool stdout/stderr;
- environment variables or secrets;
- complete shell commands;
- arbitrary Host-provided launch data.

history records carry the independent `AgentSessionKey`; display-time
grouping may group visually by project/runtime but may never combine the
underlying transition arrays.

## 10. observability

new structured log fields:

```text
agent.runtime
agent.session_hash
agent.native_event
agent.kind
agent.state
agent.event_id
agent.adapter_version   (deferred: schema v1 carries no adapter
                         version field; lands with a v2 wire field)
agent.notification_queued
```

raw session IDs and cwd are not logged; use a stable process-local hash
and project basename. Adapter Health exposes:

- runtime and adapter version;
- available / partial / unavailable;
- declared capabilities;
- last accepted event time;
- last bounded error category;
- setup compatibility message.

delivery failure never changes a provider process's exit status.

## 11. security invariants

- both ingestion endpoints remain loopback-only;
- local-process spoofing remains accepted by the product's single-user
  trust boundary;
- `/agent/events` accepts data, never executable behavior;
- the overlay remains receive-only and `capabilities/default.json`
  remains byte-for-byte unchanged;
- Settings commands remain allowlisted and window-label guarded;
- adapters are fail-open observers and never answer permissions;
- Host focus uses code-owned allowlists only;
- raw provider payloads do not cross into frontend IPC or persistence.

the runtime names are a narrow compatibility exception to the project's
third-party naming rule: neutral names may appear in adapter IDs, setup
documentation, tests/fixtures, and UI labels. no third-party logos,
assets, copied trade dress, or implied affiliation.

## 12. deletion matrix

v7 is not complete while any active cmux-specific product surface
remains:

| surface | required change |
|---|---|
| `SourceKind::Cmux` and tests | migrate to `Agent` |
| `/notify` `RequestSource::Cmux` | remove |
| `cmux_priority`, `cmux_ttl_secs` | migrate/alias, serialize new names |
| rotation defaults/healing | replace Cmux position with Agent |
| `hooks/notchtap-cmux-hook.sh` | delete |
| cmux handling in `notchtap` | delete |
| `CmuxSection.tsx` and Settings tab | replace with Agents |
| frontend source labels/fixtures/styles | replace with Agent model |
| active architecture/testing/roadmap text | mark historical relay superseded |
| static prototypes deliberately kept current | update or explicitly archive |

Git history is the recovery mechanism. do not preserve a dormant cmux
adapter or compatibility UI.

## 13. test contract

full cases live in `TESTING_STRATEGY.md` §4.13. minimum release gates:

- fixture mapping for every supported native event and every declared
  capability;
- registry transition, ordering, independent identity/history,
  duplicate, stale-sequence, stale-timeout, and retention tests;
- endpoint validation/bounds/Host-defense tests;
- noteworthy-event → Notification mapping and queue-full independence;
- `AgentState::dedup_eq` tests proving clock-only changes do not publish;
- config/history migration tests from every cmux legacy field/value;
- frontend resting/expanded/multi-session tests;
- Adapter Health and setup UI tests;
- manual real-session smoke test for all four runtimes;
- manual T3 Code smoke test for at least Codex and Claude Code;
- manual overlay scroll/pointer pass-through test on mac mini and
  notch geometry test on macbook;
- `cargo test`, `npx vitest run`, typecheck, lint, and build all green.

## 14. implementation order

1. normalized model, registry, and pure provider fixture parsers;
2. `/agent/events`, notification mapping, and `agent-state`;
3. config/history migration plus complete cmux deletion;
4. shared helper and the three command-hook adapters;
5. OpenCode plugin;
6. Agent Board resting state, then expanded/scroll interaction;
7. Settings Agents section and Adapter Health;
8. validated Open/Focus shortcut;
9. real-provider/T3/macOS manual verification and active-doc closeout.

do not ship an adapter before its capability declaration and redacted
fixture suite agree. do not call v7 complete while a runtime is shown as
fully supported based only on synthetic fixtures.

## 15. why MCP is not the transport

MCP tools are invoked by the model/client during an agent turn. the
events notchtap needs—permission waiting, idle/input waiting, completion,
failure, and session lifecycle—must be reported proactively by the
runtime even when no model tool call is occurring. provider hooks and
OpenCode's event plugin are therefore the deterministic lifecycle
transport.

the MCP community is still defining proactive triggers/events as a
separate capability ([Triggers and Events working
group](https://modelcontextprotocol.io/community/working-groups/triggers-events)).
v7 does not create an MCP server merely to wrap the local endpoint.
future MCP support is justified only if a runtime offers no hook/event
surface but does expose a standardized proactive event channel, or if
notchtap later adds model-invoked query/control tools as a separately
approved feature.
