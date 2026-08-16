# mac-notification-nudge — architecture & decisions

macos only. no windows/linux target, ever. independent, clean-room
build — not a fork or clone of any specific third-party app; no
external branding, code, or assets are used.

decisions recorded here are locked — don't re-litigate without the
operator explicitly reopening one.

---

## 1. what this is

a background utility that:

- runs a notification queue engine, permanently, as a menu-bar/notch app
- accepts manual pushes from the command line, normalized coding-agent
  lifecycle events from provider-native Agent Adapters (§10), and
  events from internal pollers (ESPN football scores, RSS news)
- renders each push as a notch-anchored overlay on the macbook, and an
  equivalent floating hud on the mac mini (no notch) — same build, both
  machines

```
┌──────────────────────────────────────────────────────────────┐
│                        input sources                          │
│ direct cli │ provider-native Agent Adapters │ internal pollers │
│  (manual)  │  (structured lifecycle hooks)  │ (football, news) │
└───────────────┬───────────────────────────┬──────────────────┘
                │                           │
                ▼                           ▼
        ┌───────────────────────────────────────────┐
        │              core engine (rust)            │
        │  - typed event bus                         │
        │  - single-slot priority queue (rotation,   │
        │    per-tier waiting lines)                 │
        │  - dispatch router                         │
        └───────┬───────────────────────────────────┘
                │
                ▼
    ┌───────────────────────────────┐
    │        presentation ui         │
    │  (react/ts webview window)     │
    │  - notch mode (macbook)        │
    │  - hud mode (mac mini)         │
    │  - animation layer             │
    └───────────────────────────────┘
```

the same core (rust) and ui (react/ts webview) run unmodified on both
machines. only one module differs: **window placement** — notch-aware
on the macbook, plain top-center hud on the mac mini. detected at
runtime via `NSScreen.main?.safeAreaInsets.top > 0` (native call,
through a thin swift shim), never at build time.

## 2. tech stack

**constraint that changes the usual calculus**: both target machines
are macos. cross-platform reach — electron's and tauri's headline
justification — isn't needed here.

**the stack: tauri (rust core + react/ts ui), with a small native
swift shim for notch geometry.**

- **footprint/perf** — tauri sits far closer to native (tens of mb, low
  idle cpu) than electron (hundreds of mb baseline, a full chromium +
  node runtime per app). matters because this runs 24/7 in the
  background.
- **dx/ecosystem fit** — the queue/animation/ui layer is the strongest
  surface (react, ts, css). rust handles polling/ipc/cli dispatch —
  bounded, real practice.
- **pure native swift** wins on every technical axis except fluency and
  iteration speed — right call for a long-term polished product, not
  for a personal tool that must keep shipping.

the ui animation layer is framer motion (`motion`) + lucide icons in
the settings window, and CSS transitions/keyframes in the overlay path;
per-event-type variety stays data (a table/stylesheet keyed by event
type), never a new render path.

**reduce-motion**: deliberately not handled, anywhere — no
`prefers-reduced-motion` media queries, no `MotionConfig
reducedMotion`, no JS gates. a standing app-wide non-goal (operator
decree, 2026-08-16): this is a personal overlay for one operator's own
machines, and its motion always plays.

## 3. cross-device behaviour

| | macbook (has notch) | mac mini (no notch) |
|---|---|---|
| window anchor | pinned over the notch cutout | top-center floating hud |
| detection | `safeAreaInsets.top > 0` at runtime | same check, fails → hud mode |
| everything else | identical — same queue, same animation, same cli input | identical |

**integration pattern — subprocess, not ffi**: the one unavoidable
native call (`NSScreen` safe-area geometry) is compiled as a tiny
standalone swift cli (`notchtap-detect`) that prints json to stdout and
exits. the rust core calls it via `std::process::Command` and parses
the output. this avoids ffi complexity, keeps the swift boundary
isolated, and keeps the pure decision function
(`fn presentation_mode(safe_area_top_inset: f64) -> Mode`)
unit-testable apart from the untestable subprocess call.

**multi-display**: the notch exists only on the built-in display. the
window uses the screen containing the menu bar; that is acceptable
behaviour for this two-machine product.

## 4. always-on background behaviour

- `LSUIElement = true` → no dock icon, menu-bar presence only.
- login item via `SMAppService.mainApp.register()` — requires macos 13+.
- menu-bar tray with a deliberately small item set: **pause** (label
  toggles to "resume"), the silence controls (timed mutes, skip
  today's Silent Period), **settings…**, and **quit**. anything richer
  than a toggle belongs in the settings window, not in more tray items.
- **always-on-top** (`NSWindowLevel` above standard windows) — a
  notification overlay buried under other windows is useless.
- **transparent overlay window** — undecorated, transparent,
  shadowless, non-resizable, never takes focus. a transparent tauri
  webview requires `macOSPrivateApi: true` in `tauri.conf.json`; the
  private-api use is an accepted cost (app-store distribution is
  already ruled out by §13, so it has no practical bite).

### 4.1 hover primitive

the overlay window sits at `NSStatusWindowLevel`, flush over the real
menu bar, with `set_ignore_cursor_events(true)` so it never swallows
clicks meant for other apps' menu-bar icons. a `tauri-nspanel` tracking
area's mouseEntered/mouseMoved/mouseExited still fire under
`ignoresMouseEvents = true` — click dispatch and tracking-area
notifications are gated by independent AppKit mechanisms (verified
empirically; see `docs/design/hover-cursor-tracking.md`, including the
rejected frontend-reported-bounds alternative). the hover machinery is:
a tracking area on the `OverlayPanel`, a pure rust rect-derivation
function (`src-tauri/src/hover.rs`), and a `hover-changed` event to the
webview — with zero change to the overlay's capability file. the one
carve-out from unconditional click-through is the icon strip (§11).

## 5. cli push and the `/notify` endpoint

**direct**: any script calls the local endpoint. always available, zero
dependencies.

**cli contract (locked)**: flags only, one form for humans and scripts —
`notchtap --title <t> --body <b> [--subtitle <s>] [--detail Label=Value]... [--port <p>]`.
no positional form. `subtitle` and repeatable `details: [{label,
value}]` pairs are optional wire fields, display-only, capped/truncated
server-side, and never influence priority/rotation. a bare
`{title, body}` payload works unchanged. port resolution: `--port` flag
→ `$NOTCHTAP_PORT` env var → `9789`. the cli is a shell script (`jq` +
`curl`) and never reads `config.toml` — if the server port is changed
in config, set `$NOTCHTAP_PORT` to match. the cli also has a `run`
subcommand: `notchtap run -- <command>` wraps a long-running command
and pushes a completion card when it finishes (successful runs under
`--min-secs` skip the push; a failure always pushes).

**default port**: `127.0.0.1:9789`. loopback only, no external
exposure. if the port is in use at startup, the core exits with a clear
error rather than silently picking another port.

scope note on that boundary: loopback-only prevents *network* exposure,
but it is not an authentication boundary between local processes —
anything running on the machine can post notifications. acceptable by
design for a single-user personal tool.

**dedup posture**: no content-hash deduplication — duplicate pushes
queue as duplicates, acceptable for trusted local sources. the
`/agent/events` endpoint (§10) has identity-level event-ID/sequence
idempotency, which is a different thing from fuzzy content dedup.

**relays are heads-up only**: an external tool forwarding its own
notifications in (e.g. the uptime-kuma recipe in
`docs/recipes/kuma-webhook.md`) can never answer back into the tool
that raised the alert.

## 6. queue model — single slot, priority tiers, rotation

the domain glossary in `CLAUDE.md` defines the terms; the shape:

- **exactly one item is ever visible** (the Slot). accepted items wait
  in three per-priority lines (`Low | Medium | High`), each
  independently capped (`max_queued_per_tier`) — a `Low` burst can't
  starve `High`'s waiting room; pushes beyond a tier's cap get `429`.
- **promotion** is priority-ordered: highest non-empty tier first, then
  the configured Rotation Order rank over Origin, then arrival order.
- **rotation, not ttl**: display time is measured from promotion. a
  `Recurring` rotation kind requeues to the back of its own tier after
  its turn; `OneShot` drops. a **Topic** gives a Recurring event a
  supersession identity — a fresh event sharing the Topic updates the
  existing card in place (waiting or visible) instead of queueing a
  second one, with a small capped extension when remaining time is low.
- **preemption**: a strictly-higher-priority arrival cuts the visible
  card short; the preempted card re-queues at the head of its own tier
  with its remaining turn intact. equal priority never preempts.
- **pause** disables promotion only. pushes are still accepted and
  buffered (the http response says so: `202` +
  `{"status": "paused", "queued": <n>}`), the visible item finishes its
  natural rotation, and resume promotes immediately — nothing dropped.
  the tray toggle is session-only; the persisted `start_paused` config
  flag (the kill switch) makes the app *launch* paused.
- **display-only**: notifications auto-dismiss by rotation; there is no
  approve/deny action. an interactive "blocks until you respond" model
  would need the agent runtimes' own permission hooks answering back —
  deliberately out of scope (§10's heads-up-only boundary) until
  explicitly requested.

every mutation of the Slot and the waiting lines flows through one
module — the Engine (`src-tauri/src/engine.rs`); queue, wake, and the
live-match handle are private to it, so its guarantees are structural
rather than convention.

## 7. silenced state & priority preemption

- **Silenced is a second quiet state, distinct from Paused, and Paused
  stays absolute.** while Silenced, Medium/Low pushes buffer exactly as
  under Paused (nothing dropped) but a High event still promotes
  (**Breakthrough**) as a compact card. Pause and the `start_paused`
  kill switch keep their show-nothing meaning — Breakthrough does not
  apply to them. rationale: one lever that is priority-gated for
  routine quiet, one that is absolute; merging them would let a spammy
  High source defeat the kill switch.
- **silence is display-only.** it affects only what promotes to the
  overlay; the HTTP contract is unchanged, pollers keep observing, and
  the idle surface (clock, Agent Board) behaves normally.
- **two silence sources, unioned**: the daily Silent Period (default
  ON, `00:00-10:00`, one window per day, local wall clock, persisted in
  the `[silence]` config block) and tray Timed Mutes (30m/1h/2h
  presets, auto-resume, session-only). Skip ends today's window early
  and re-arms at the next window start. calendar-driven silence was
  rejected: it would pull OAuth/network into a loopback-only app.
- **expansion is a Medium/High privilege**: Low promotions and
  Breakthrough promotions start compact; the manual expand hotkey still
  grows any visible card.
- **default priorities**: football goals deliberately High — a goal
  preempts the screen and breaks silence by explicit operator choice.
  manual Medium, news Low, agent permission/input/failure High,
  completion Medium; all per-source configurable.

## 8. configuration

`~/.config/notchtap/config.toml`, read exactly once at startup.
**restart is the reload mechanism** — no file-watcher, no hot-reload,
ever. the settings window (§9) is the editing surface; the file is the
storage. boot runs the loaded config through the settings window's
`validate()` and logs each violation as a warning, continuing with the
file's values; malformed TOML still fails fast in `Config::load`.

`detect_path` (default `/usr/local/bin/notchtap-detect`) is an absolute
path because gui/login-item-launched apps get a minimal `PATH` — the
core never does a `PATH` lookup. `detect_path` is file-only, not
editable from the settings window.

`~/.config/notchtap/crests/` is the one binary-asset cache: club crest
PNGs fetched at runtime from ESPN's scoreboard-provided logo URLs
(https + espncdn hosts only), one attempt per team per process
lifetime, no eviction (bounded by the watched leagues' team counts),
served to the overlay via tauri's asset protocol scoped to that
directory alone.

**football (espn) decisions**: default leagues `eng.1`,
`uefa.champions`, `esp.1` — changing leagues is a config edit, never
code. trigger scope is every scoreboard delta espn reports (goals,
kickoff/half-time/full-time, cards where the payload carries them);
narrowing is a filter change, not a redesign. the opt-in
`espn_live_card` flag flips a live match into a single updating card:
Topic identity `espn:{league}:{match_id}`, `Recurring` while in play,
the full-time event emitted `OneShot` on the same Topic so the card
retires through the ordinary path — no bespoke teardown. multi-match
stays unarbitrated: each concurrent match gets its own Topic and shares
the tier via ordinary rotation-order/FIFO.

**news (rss) decisions**: news items are overlay-only by design — they
never leave the machine. feeds plus `rss_topics` (Google News query
feeds) are config; polling is opt-in.

## 9. ipc security model — two windows, two trust levels

the frontend is **untrusted code running in a webview** — even though
it's first-party. the rust core treats it as a display-only consumer.

**the overlay window (`main`) is receive-only, permanently:**

- it receives events via tauri `emit`/`listen` and invokes nothing.
- `capabilities/default.json` grants listen-only event permissions
  (`core:event:allow-listen`/`allow-unlisten` — not
  `core:event:default`, which would also grant emit), no filesystem, no
  shell, no network. **it must never change.**
- the webview csp restricts `connect-src` to tauri's ipc endpoints —
  the frontend cannot `fetch()` the local `/notify` endpoint or
  anything else. a `csp: null` config would leave that path open even
  with locked-down capabilities.

**the settings window (`settings`) is the one exception, opt-in-gated,
not default-safe.** tauri v2 allows app-defined commands to *every*
window by default; the capability file does not gate them unless the
commands are opted into the acl. the receive-only overlay guarantee
rests on three legs, all required:

1. **`build.rs` opt-in** — `tauri_build::AppManifest::commands(&[...])`
   turns allow-all into deny-by-default. `src-tauri/build.rs` is the
   authoritative command list. **never add a new `#[tauri::command]`
   without adding it to that list** — otherwise it silently becomes
   callable from the overlay window too.
2. **dedicated capability** `src-tauri/capabilities/settings.json` —
   `"windows": ["settings"]`, granting the autogenerated `allow-*`
   permission per command plus `core:event:allow-listen`/`allow-unlisten`.
   `capabilities/default.json` is not touched.
3. **defense-in-depth label check** — every command takes the calling
   window and rejects any label other than `settings`
   (`ensure_settings_window`). the acl should make this unreachable; it
   exists because the acl doesn't protect against handler scope bugs,
   and a `generate_handler` edit that forgot the `build.rs` list would
   otherwise fail open.

the settings-window commands (all in `src-tauri/src/settings.rs` and
siblings; `build.rs` is the authority if this list drifts):

| command | purpose |
|---|---|
| `get_config` | the **booted** config (managed state), not a fresh file read |
| `get_default_config` | `Config::default()` — single source of truth for "Reset to defaults" |
| `save_config_and_relaunch` | validate → atomic write (same-dir temp file + rename — a half-written file is a bricked boot) → relaunch; the `Err` arm carries per-field messages |
| `set_appearance` | validates range, atomically writes `[appearance]`, updates managed state, emits `appearance-changed` to the overlay — the one live-apply path |
| `send_test_notification` | canned per-source test event through the same path `/notify` uses |
| `send_agent_test_event` | synthetic agent event through the `/agent/events` path |
| `get_agent_health` | per-runtime Adapter Health snapshot |
| `get_queue` / `clear_queue` / `skip_current` | waiting-line visibility and controls |
| `get_history` / `clear_history` | notification history (JSONL store) |
| `get_recent_log_lines` | read-only log tail panel |
| `search_news_now` | one-shot news poll for the configured topics |
| `get_about_info` | version/bundle/system stats card |

the secrets commands (`set_secret`, `get_secret_status`) are removed
along with the secrets store — no command carries a secret across ipc
in either direction.

**save & relaunch**: config is read once at boot, so saving validates
rust-side, writes atomically, then relaunches the app. no hot-reload
plumbing (the `set_appearance` live-apply is the deliberate, narrow
exception).

**clicks do not imply invoke.** the overlay reacts to clicks on the
icon strip (§11), but the frontend still never talks to rust: a native
`NSEvent` local monitor on the rust side observes the mouseDown,
decides what it hit, and pushes a typed event down the same
receive-only channel. wanting an `invoke` for a click is the signal
you're about to break this boundary, not a gap to fill.

## 10. agent integrations and the Agent Board

the product's coding-agent focus, without coupling to a terminal or
IDE. the Agent Runtimes are Claude Code, Codex, Kimi, and OpenCode;
each integrates through its own documented lifecycle hooks/plugin and
normalizes into one Agent model. T3 Code needs no special adapter: it
launches ordinary runtimes, so their normal hook configuration remains
the integration surface.

- **one Origin, separate Runtime and Host**: every adapter-produced
  event has Origin `Agent`. Runtime selects compatibility and
  presentation policy. optional Host metadata (T3 Code, terminal, IDE)
  exists only for display and Open/Focus behavior — never identity,
  never Rotation Order.
- **independent sessions forever**: identity is Runtime + the
  provider's native session ID. project path is metadata, never
  identity. two session histories are never merged.
- **capability-declared adapters**: partial provider support is valid
  and visible. the UI omits unsupported data rather than inferring it
  from notification wording. Kimi support is version-gated; Codex's
  undocumented input/terminal-failure gaps stay declared gaps.
- **hooks, not MCP**: lifecycle delivery is proactive and
  deterministic. there is no notchtap MCP server; an MCP control plane
  is reconsidered only for a separately approved model-invoked use
  case.
- **heads-up only**: Permission Requested and Input Required are
  high-priority heads-up states. notchtap never approves, rejects,
  replies, launches, supervises, or scrapes the runtime. Open/Focus
  Session goes only through code-owned Host allowlists.
- **two presentation paths**: noteworthy Agent Events use the existing
  queue/Slot. session lifecycle/progress updates the separate
  rust-owned Agent Registry and Agent Board without creating a card per
  tick.
- **Agent Board**: when the Slot is empty, live/retained sessions take
  precedence over ordinary idle content. resting shows the
  highest-ranked session richly and represents every other session
  individually; hover expands to a screen-bounded scrollable list.
  ordering is urgency first, FIFO within equal urgency. terminal
  retention is configurable (default ten minutes); waiting states do
  not expire like notifications.

### 10.1 `POST /agent/events` — the ingestion contract

provider hooks do not post to `/notify` — session updates are not
notifications, and overloading the old schema would discard the
state/identity contract. the endpoint reuses `/notify`'s listener,
loopback binding, Host-header defense, body-limit posture, and logging.

schema v1 (normalized by the adapter, never a raw provider payload):

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
  "details": [{ "label": "Tool", "value": "shell" }],
  "capabilities": ["session_lifecycle", "permission_requests"],
  "project": { "name": "notchtap", "cwd": "/path" },
  "host": { "name": "T3 Code", "bundleId": "allowlisted-value" },
  "subagent": { "id": "native-id", "label": "test runner", "state": "working" },
  "terminal": false
}
```

`sequence`, project, host, subagent, summary, and details are optional.
validation and bounds (hard caps centralized in `agents/adapter.rs`):
unknown `schemaVersion`, malformed JSON/enum, absent identity, or
unsupported runtime → `400`; oversized body → `413`; accepted → `202`;
duplicate `eventId` or stale sequence → idempotent `202` with no
registry/notification change; internal failure → `500`. body 64 KiB;
IDs 256 bytes; summary 500 scalars; names/labels 120; cwd/detail
values 1,024; details 12; capabilities 16; 50 retained transitions per
session; 2,048 remembered event IDs (LRU). strings are trimmed and
control characters removed before storage or rendering. secrets,
prompts, raw tool input/output, environment values, and complete
command lines are never forwarded — adapters may extract a safe tool
name, a basename, and a short human summary. with a `sequence`, lower
or equal than last accepted is stale; without one, receive order is
authoritative and `eventId` supplies duplicate protection.

### 10.2 adapter delivery

a small rust binary, `notchtap-agent` (`hook <runtime>`, `test`,
`status`, `doctor`), is the shared delivery helper for Claude Code,
Codex, and Kimi; OpenCode uses a TypeScript plugin
(`adapters/opencode/notchtap.ts`) because its lifecycle surface is a
plugin event bus — same schema, limits, sanitization, and fail-open
semantics. delivery rules:

- connect/read timeout at most 750 ms;
- **fail open**: provider sessions are never blocked by notchtap
  absence or malformed optional data; exit 0 even on delivery failure,
  writing a bounded diagnostic to the adapter log, never stdout;
- no decision JSON, approval answer, or mutation of the native event;
- no daemon, background supervisor, shell interpolation, or `jq`
  dependency; `NOTCHTAP_PORT` is the port override.

`doctor` is read-only: it inspects the runtimes' hook config files and
reports which expected hook events are wired and whether each command
string resolves to an executable — it never creates, edits, or repairs
them. setup ownership stays with the user: Settings shows
detected/undetected status, setup snippets and exact target file, a
test event, last-seen time, declared capabilities, and uninstall
instructions; notchtap never silently edits a user's global provider
configuration.

### 10.3 security invariants

- both ingestion endpoints remain loopback-only; local-process spoofing
  stays inside the single-user trust boundary;
- `/agent/events` accepts data, never executable behavior;
- the overlay remains receive-only; `capabilities/default.json` remains
  byte-for-byte unchanged;
- settings commands remain allowlisted and window-label guarded;
- adapters are fail-open observers and never answer permissions;
- Host focus uses code-owned allowlists only;
- raw provider payloads never cross into frontend IPC or persistence.

runtime names are a narrow compatibility exception to the project's
third-party naming rule: neutral names may appear in adapter IDs, setup
docs, tests/fixtures, and UI labels. no third-party logos, assets,
copied trade dress, or implied affiliation.

## 11. tab-notch pull model: the icon strip

design spec: `docs/superpowers/specs/2026-08-02-tab-notch-design.md`.
the shipped implementation under `src/` is authoritative.

- **the notch is pull-based, additively.** push behaviour (interrupts,
  rotation, priority preemption) is completely unchanged and takes
  precedence over everything pull-related, without exception. tab
  selection decides what the notch shows when the operator goes
  looking; it never decides what the notch is allowed to tell them.
- **rest is bare**: shell + idle face only. icons exist only inside a
  hovered flank, hidden with `visibility: hidden` (NOT `display:
  none`) — that choice is load-bearing twice: it keeps the strip's
  width reserved, and it means every `infinite` icon animation MUST be
  gated on `.hovered` or it ticks forever behind an invisible node.
- **selection: max one, or none**, remembered across hovers, cleared if
  its source stops being live. hover always shows the selected tab's
  card in COMPACT form and never auto-expands — expansion is a
  deliberate, separate keyboard act. this is the single most important
  behavioural line in the feature.
- **click detection is rust-side, by necessity not preference.** the
  overlay's capability file grants event listen/unlisten and nothing
  else — no invoke, no emit — so a click the WEBVIEW sees has no
  channel back to the rust side that owns the selection. a native
  `NSEvent` local monitor (`src-tauri/src/click.rs`) observes the
  mouseDown, hit-tests it against `hover::icon_strip_rects`, and pushes
  the result as a typed `tab-selection-changed` event, mirroring
  `hover-changed`. **do not "simplify" this to a webview onClick** — it
  cannot work without reopening the capability file, which §9 forbids.
- **`set_ignore_cursor_events` is not unconditionally true.** it opens
  exactly while the strip is the live hover target (hovered ∧ slot
  idle) and reverts on hover-exit or slot promotion. the API is
  WINDOW-granular, so "only clicks inside the strip's rect count" is
  enforced by the monitor's hit-test, not by the toggle.
- **the prefix keymap grabs BARE keys, temporarily.** arming registers
  unmodified keys (source digits, `[`, `]`, enter, o, p, esc)
  system-wide for a 2s window. this is genuinely dangerous — a failed
  RELEASE leaves a bare `Enter` grabbed across the whole machine — so
  it carries an unconditional watchdog that ignores the generation
  counter, a release on `RunEvent::Exit`, and ERROR-level logging on
  any failed release. a failed *register* is benign; a failed
  *unregister* is not. never register these outside a live armed
  window.
- **hard non-goals**, standing project rules rather than oversights: no
  `prefers-reduced-motion` handling (an app-wide non-goal, §2) and no
  accessibility variants in this feature; HUD-mode/mac-mini scope, with
  real notch-hardware verification operator-owed; no breaking-news
  interrupts (news stays pure pull).

## 12. logging & observability

- **rust core**: `tracing`, writing a rotating log at
  `~/Library/Logs/notchtap/notchtap.log` (`src-tauri/src/logging.rs`),
  10 mb × 3 backups, `info` in release, `debug` in dev. this is a
  background app — when something breaks, the user needs a log to read.
- **frontend errors**: devtools-only by design — the overlay is
  receive-only, so no tauri command carries them back to the log file,
  and none should be added.

## 13. distribution / install

no apple developer program fee needed for personal use:

- the ios 7-day reinstall pain is specific to ios sideloading
  provisioning profiles. **macos has no such limit.**
- a mac app built locally and run on the machine that built it never
  gets the `com.apple.quarantine` flag, so gatekeeper's notarization
  check doesn't apply. a free apple id can ad-hoc sign it.
- moving between the two machines: build from source on each, or copy
  the built `.app` via a non-quarantining method (local share, usb,
  `scp`); `xattr -cr YourApp.app` clears a stray flag.

the $99/yr would be needed only for distributing to other people, the
app store, or paid-team entitlements — none apply. **the app store is
the wrong channel regardless of cost**: sandboxing blocks the
persistent overlay, and review rejects apps that mimic system chrome.

**minimum macos version**: 13 (ventura) — `SMAppService` is unavailable
earlier.
