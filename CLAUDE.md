# notchtap repository guide

Canonical guidance for every coding agent and maintainer working in this repository.

## project state

scaffolded and shipping through v6. per-plan history is NOT duplicated
here — read `plans/done/` (filed, one file per plan) and `git log` for
what landed when. the notes below are only the things those sources
don't tell you.

- the docs folder isn't part of the app build; the tauri/rust/web
  project lives at repo root alongside `docs/`.
- `src/` is the authoritative UI implementation. `prototype/*.html` is
  the sole static visual reference: it mirrors shipped surfaces, is not
  wired into the build, and must never contain proposals or experiments.
- `.claude/skills/` is the single checked-in skill tree;
  `.agents/skills` is a tracked compatibility symlink to it. never copy
  skills between tool-specific directories. machine-local AI settings,
  credentials, caches, and worktrees stay ignored.
- **repo cleanup, 2026-08-03** — removed, not lost. everything below is
  in `git log` and can be restored with `git checkout <commit> -- <path>`
  if it's ever wanted again; nothing here was consumed by the app build,
  which is why the removals are safe:
  - `prototypes/` (plural) — proposal/scratch mocks, including the two
    r3 tab-notch design mocks. those two are also still on branch
    `feat/tab-notch-redesign`. the tab feature has shipped, so `src/`
    is authoritative over them now; the spec that cites them
    (`docs/superpowers/specs/2026-08-02-tab-notch-design.md`) says so.
  - `DESIGN.html` — the old system-level token/law reference. it
    predated the Agent Board and had gone stale;
    `prototype/index.html` + `vendor/shared-ui/design/tokens.css` are
    the live equivalents.
  - `.mcp.json` + `mcp-servers/` — project-local MCP config and an
    unused research server. the PAL server this repo actually uses is
    configured at USER level (`~/.claude.json`) and is unaffected.
    `.mcp.json` is now gitignored, so a local one can be recreated
    without it landing in a commit.
  - `skills-lock.json`, `assets/branding/build_branding.py`, the
    unreferenced weather glyphs, and the ios/android/windows icon sets
    tauri never bundles (`tauri.conf.json` names only the five mac/win
    icons that remain).
- the test suite must stay green (`cargo test` from `src-tauri/`,
  `npx vitest run` from repo root, all gated by ci). current test
  counts live in `docs/TESTING_STRATEGY.md` §0 and only there — don't
  restate them anywhere else.
- **`SlotState::dedup_eq` rule:** continuously-varying wire fields
  (e.g. `ttl_ms`/`remaining_ms`) must extend `dedup_eq` explicitly and
  must never rely on derived `PartialEq` — deriving it causes every
  tick to read as a content change.
- the hover primitive shipped (tracking area, rust-derived card rect,
  `hover-changed` event) and all four hover CONSUMER features shipped
  in plan 093: TTL-bar hover-pause, idle weather peek, scorecard
  reveal-on-hover, idle expand-on-hover.
- the frontend toolchain spike (TypeScript 7 / Vite 8 / Vitest 4)
  returned a **GO** verdict, but nothing was adopted — `package.json`
  is untouched and adoption is a separate unwritten plan. don't read
  the GO as "already done".
- remaining open work: the manual checklist rows in
  `docs/IMPLEMENTATION_PLAN.md` §6, and whatever `plans/` holds.

<!-- trimmed 2026-07-21: a plan-by-plan changelog (v1–v6, plans
037–087) lived here. it was reconstructible from plans/done/ and git
log, so it was cut to keep this file cheap to load every session. -->

`docs/archive/BLIND_REVIEW.md` and `docs/archive/CHANGES_SUMMARY.md` were
changelog/audit artifacts from the planning pass, not sources of
truth — the decisions they describe are already folded into the three
docs below. `docs/archive/V1_TECHNICAL_SPEC.md`,
`docs/archive/V2_TECHNICAL_SPEC.md`, and `docs/archive/V3_TECHNICAL_SPEC.md`
were likewise archived: those phases shipped. all five were removed at
repo close-out (2026-07-23), retrievable via `git log -- docs/archive/`.
`docs/V3_6_TECHNICAL_SPEC.md`, `docs/V5_TECHNICAL_SPEC.md`, and
`docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` are the active
working-draft specs now.

the dev machine is the mac mini (no notch), user `chetanjain`, home
`/Users/chetanjain`; the rust toolchain is installed. notch-mode
behaviour still needs per-change verification on the macbook.

## source of truth

this file is the single physical repository guide. `AGENTS.md` and
`CONTEXT.md` are compatibility symlinks to it for tools that discover
those conventional names. the **domain glossary** at the end defines
terms like Promotion, Visible/Waiting, Paused, and Presentation Mode;
keep code and docs consistent with it.

`docs/ARCHITECTURE.md` holds the locked decisions (scope phasing, tech
stack, cross-device behaviour, distribution model) — do not re-litigate
these without the user explicitly reopening them. `docs/IMPLEMENTATION_PLAN.md`
holds the phased build sequence and exit criteria through v7.
`docs/TESTING_STRATEGY.md` holds the testing approach — frameworks, what's
tdd'd first vs written after, per-component test plan, and what's
deliberately left as manual-only verification. read all three before
starting implementation work.

`docs/V3_6_TECHNICAL_SPEC.md`, `docs/V5_TECHNICAL_SPEC.md`, and
`docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` are v0 drafts that
operationalize those three into code-level specifics for
the currently-active phases — exact file layout, struct/type shapes,
the `/notify` json schema, the `notchtap-detect` subprocess contract,
config/logging paths, error-to-status-code mapping. unlike
`ARCHITECTURE.md`, neither is locked — adjust them freely as
implementation surfaces friction. if a change there is actually a
*decision* change (a default, a scope boundary), make that edit in
`ARCHITECTURE.md` instead. the equivalent v1/v2/v3 specs were archived
at `docs/archive/` — those phases already shipped, so they were historical
records, not active contracts (same status as `BLIND_REVIEW.md`/
`CHANGES_SUMMARY.md` above). all five were removed at repo close-out
(2026-07-23); retrievable via `git log -- docs/archive/`.

## commands (once scaffolded)

standard invocations (`npm run tauri dev`, `npx tsc --noEmit`,
`npx vite build`, `cargo build`/`cargo test` from `src-tauri/`,
`npx vitest run`) are in `package.json` and the `justfile` — read those.
the non-obvious ones:

- `npx biome check .` is the local dev command (`npm run lint:fix`
  auto-applies), but the enforcing gate CI and `just check-web` run is
  `npx biome ci .` — they are not interchangeable.
- `./notchtap --title "t" --body "b"` — manually trigger a notification
  against the local `/notify` endpoint (default `127.0.0.1:9789`,
  override via `--port` or `$NOTCHTAP_PORT`), for testing the
  queue/animation without a real event source. the cli is a committed
  shell script at repo root; besides flags, it also has a `run`
  subcommand (plan 058) — `notchtap run -- pnpm build` wraps a
  long-running command and pushes a completion card when it finishes
  (skipped for successful runs under `--min-secs`, default 15s; a
  failure always pushes)
- `just test-all` — one-command local verification mirroring
  `.github/workflows/ci.yml` exactly (see `justfile` at repo root for
  the full recipe list: `setup`, `dev`, `test-rust`, `check-rust`,
  `test-web`, `check-web`, `audit-web`, `build-web`, `check-cli`,
  `check-swift`, `build-media-adapter` — that last one compiles the
  vendored MediaRemote framework and installs it under
  `~/Library/Application Support/notchtap/`, no sudo). on a fresh
  clone, run `just setup` (`npm ci`) first —
  `test-all` does not install web deps for you.
  `just push "title" "body"` wraps the `./notchtap` cli call above.
  `just` is not installed on the dev machine yet — `brew install just`
  first.

`cargo test` and `npx vitest run` should both be clean before any phase
in `docs/IMPLEMENTATION_PLAN.md` is marked done — see that doc's §6 and
`docs/TESTING_STRATEGY.md` §7. there's no repo-wide coverage percentage
gate (see `docs/TESTING_STRATEGY.md` §6 for why) — the bar is "every example
case listed for the phase's components has a passing test," not a
coverage number. physical-hardware behaviour (notch geometry, hud
placement, animation look) stays a manual checklist by design —
`docs/TESTING_STRATEGY.md` §5 explains why those specific things aren't
worth automating.

## architecture (once scaffolded)

this is a tauri app: a rust core plus a react/ts webview ui, not
electron and not pure native swift (see `docs/ARCHITECTURE.md` §8 for why).

- **rust core** (`src-tauri/src/main.rs`) owns a local http listener on
  `127.0.0.1:9789` (`/notify`), a typed event bus, a fifo notification queue
  (capped concurrent visible items, per-item ttl), and window
  positioning. this is the only process that talks to the outside
  world (cli pushes, internal pollers, and v7's loopback Agent Adapter
  events).
- **react/ts frontend** owns rendering only. two vite entries: the
  overlay (`index.html` → `src/App.tsx`, `src/styles.css`) and the
  settings window (`settings.html` → `src/settings/`), see
  `vite.config.ts`. the overlay
  receives queued events via tauri's event system
  and renders them through an animation template. v1 has exactly one
  template (enter/hold/exit); v2 replaces this with a config table
  keyed by event type — that should stay a data change, not a new
  render path.
- **cross-device behaviour is a single runtime branch, not two
  builds.** the same compiled app runs on both the notch macbook and
  the notchless mac mini; a runtime check
  (`NSScreen.main?.safeAreaInsets.top > 0`) decides whether the window
  anchors over the notch cutout or floats as a top-center hud. do not
  fork this into separate build targets.
- **the swift↔rust boundary is a subprocess, not ffi.** the
  `NSScreen` check lives in a standalone swift cli (`notchtap-detect`)
  that prints json to stdout; the rust core shells out to it via
  `std::process::Command` and parses the result (`docs/ARCHITECTURE.md`
  §5). keep the pure decision logic (`fn presentation_mode
  (safe_area_top_inset: f64) -> Mode`) separate from that subprocess
  call — the function is unit-testable, the subprocess call is not
  (`docs/TESTING_STRATEGY.md` §4.4).
- **v1 has no approve/deny action.** notifications are display-only,
  auto-dismissed by ttl. do not add a "respond back into the agent
  cli" loop without reading `docs/ARCHITECTURE.md` §20 first — that
  requires the agent cli's own permission/pre-tool hooks, which is a
  deliberately separate, harder problem, out of scope until explicitly
  requested.

## naming

this project has no association with, and does not use, any third-party
app's branding or code. use the product name (`notchtap`), this repo's
own name (`mac-notification-nudge`), or generic terms by default. v7's
one narrow exception: supported coding-runtime names (Claude Code,
Codex, Kimi, OpenCode) may appear neutrally in adapter identifiers,
setup docs, tests/fixtures, and UI labels because compatibility requires
them. no third-party logos/assets, copied trade dress, or implied
affiliation.

## ipc & security (once scaffolded)

tauri v2 uses a capabilities/permissions system. the frontend in this app
is **receive-only** in v1 — it listens for a single custom event from
the rust core (`notification-promoted`, emitted exactly once per item
at promotion time, never at enqueue) and renders it. there are no
frontend-to-rust invoke commands in v1.

the `src-tauri/capabilities/default.json` should be locked down to the
minimum: one permission for the custom event channel, no file-system
access, no shell access, no network access from the frontend. the
frontend should not be able to trigger notifications — only display what
the rust core sends it.

**v5 settings window is the one exception, and it's opt-in-gated,
not default-safe.** tauri v2 grants app-defined commands to *every*
window by default — the settings window's seventeen invoke commands
(`clear_history`, `clear_queue`, `get_about_info`, `get_agent_health`,
`get_config`, `get_default_config`, `get_history`,
`get_queue`, `get_recent_log_lines`, `get_secret_status`,
`save_config_and_relaunch`, `search_news_now`, `send_agent_test_event`,
`set_secret`, `send_test_notification`, `set_appearance`,
`skip_current`) are scoped to it alone only because `src-tauri/build.rs`
opts into `tauri_build::AppManifest::commands(&[...])`
(deny-by-default) plus a dedicated `capabilities/settings.json`. never add a new
`#[tauri::command]` without adding it to that `build.rs` list —
otherwise it silently becomes callable from the overlay (`main`)
window too, breaking the receive-only guarantee above.
`capabilities/default.json` must never change. full contract:
`docs/V5_TECHNICAL_SPEC.md` §2.

**plan 171 added a CLICK path without touching any of the above — read
this before assuming a click implies an invoke.** the overlay reacts to
clicks on the icon strip, but the frontend still never talks to rust: a
native `NSEvent` local monitor (`src-tauri/src/click.rs`) observes the
mouseDown on the rust side, decides which icon it hit, and pushes a
typed `tab-selection-changed` event down the same receive-only channel
`hover-changed` already uses. this is not a stylistic preference — the
overlay's capability file grants event listen/unlisten and NOTHING else,
so a click the webview sees has no way to tell rust about it. if you
ever find yourself wanting an `invoke` for a click, that is the signal
you are about to break the boundary, not a gap to fill. relatedly,
`set_ignore_cursor_events` is no longer unconditionally `true` — see
`docs/ARCHITECTURE.md` §22 for exactly when it opens and why the
hit-test, not the toggle, is what narrows clicks to the strip.

## rust error handling

- **library/internal modules** (queue, event bus, event types): use
  `thiserror` for structured, matchable error variants. tests should be
  able to assert `matches!(err, MyError::QueueFull)`.
- **application boundary** (main.rs, HTTP handlers, CLI entrypoint): use
  `anyhow` for ergonomic error propagation. the HTTP layer returns
  specific status codes (400 for malformed json, 429 for queue full,
  500 for unexpected), but the internal error type doesn't need to leak
  into every function signature.

this split is standard in the rust ecosystem and matches the testing
strategy: unit tests match on `thiserror` variants; integration tests
assert on HTTP status codes.

---

## domain glossary

- **Event** — one incoming push (title + body, plus type/priority/
  rotation assigned by the engine). the unit that flows through the
  system.
- **Notification** — an Event being (or waiting to be) displayed.
  every Notification is an Event; "Notification" is the word for the
  display-side view of it.
- **Slot** — the single Visible position (v3.6; replaces the 3-item
  "Visible... ordered as a stack" model). there is never more than one
  Notification on screen at a time.
- **Visible** — the Notification currently occupying the Slot, if any
  (at most one, see **Slot**).
- **Waiting** — Notifications accepted but not yet shown, ordered
  **within their own Priority tier** by Rotation Order first and
  arrival order (FIFO) as the tie-break (v3.6: Low/Medium/High are
  three separate lines, not one; v6 added Rotation Order ahead of pure
  FIFO). capped per tier (`max_queued_per_tier`); pushes beyond a
  tier's own cap are rejected, independent of the other two tiers.
- **Priority** — `Low | Medium | High` on every Event (v3.6),
  independent of `EventType` — not every high-priority thing is a
  score. governs Promotion order and **Preemption**: higher-priority
  Waiting items are promoted next, and a strictly-higher-priority
  arrival cuts the currently-Visible item short — the preempted card
  re-queues at the head of its own tier with its remaining turn
  intact, and shows again once every higher-priority card has
  finished. equal priority never preempts; it waits its turn.
  (pre-silence contract was no-interruption ever; rewritten with the
  Silenced work, 2026-07-27.)
- **Origin** — which source category produced an Event (v6): `Football |
  News | Manual | Agent | Weather`. orthogonal to Priority and `EventType` —
  a source's Origin
  never changes, but its Priority is user-configurable per source. the
  only thing Origin governs is Rotation Order.
- **Agent Runtime** — the coding agent that produced an Event whose Origin
  is **Agent**, initially `Claude Code | Codex | Kimi | OpenCode`. Runtime
  identifies the producer for presentation and runtime-specific policy; it
  does not create a separate Origin or Rotation Order category.
- **Agent Adapter** — the heads-up-only bridge from one Agent Runtime's
  lifecycle hooks into notchtap. an Agent Adapter translates the runtime's
  native event into an Agent Event; notchtap does not launch, supervise, or
  scrape the runtime, and the adapter never answers on the user's behalf.
- **Agent Adapter Capability** — one kind of structured information or
  behavior an Agent Adapter can provide, such as completion, failure,
  permission requests, progress, tool details, subagents, or opening the
  originating session. adapters declare their capabilities; partial support
  is explicit, and unsupported information is omitted rather than invented.
- **Agent Event Kind** — the runtime-independent meaning assigned by an
  Agent Adapter: `Permission Requested | Input Required | Completed |
  Failed | Informational`. runtime-native event names remain diagnostic
  detail, not presentation branches.
- **Agent Session** — one active coding-agent session reported by an Agent
  Adapter. its continuously updated state can appear on the idle surface
  without producing a Notification for every progress tick; only noteworthy
  Agent Events enter the Slot. every session has an independent identity and
  history, even when multiple sessions share an Agent Runtime or project;
  histories are never merged. identity is the Agent Runtime plus its native
  session identifier; an adapter may provide a degraded process/start-time
  fallback, but a project path alone is never a session identity.
- **Agent Session State** — the runtime-independent lifecycle of an Agent
  Session: `Starting | Working | Waiting For Permission | Waiting For Input |
  Completed | Failed | Stale`. adapters translate native lifecycle names into
  these states. Informational is an Agent Event Kind, not a Session State;
  Stale means reporting ended without a clean terminal event.
- **Agent Registry** — the in-process store of every live and recently
  terminal Agent Session, keyed by Agent Runtime plus native session
  identity. it applies Agent Events to advance Session State, enforces
  Agent Session Order and Terminal Retention, and is the source the Agent
  Board and Origin::Agent Notifications both read from; it updates
  independently of whether the corresponding Notification can enter the
  Slot.
- **Adapter Health** — the presented status of one Agent Adapter:
  available, partial, or unavailable, alongside its declared Agent Adapter
  Capabilities, last-accepted-event time, and any compatibility note. it
  reflects whether the adapter is actually delivering events, not whether
  the underlying Agent Runtime happens to be running.
- **Agent Host** — the application presenting an Agent Session, such as T3
  Code, a terminal, or an IDE. Host is optional presentation and open/focus
  metadata; it is not part of Agent Session identity or an Origin.
- **Agent Session Order** — urgency first, then arrival order among sessions
  at equal urgency. a session needing input or permission ranks ahead of a
  passive Working session; equal-urgency sessions remain FIFO.
- **Terminal Retention** — how long a terminal Agent Session remains on the
  Agent Board before moving to its history: configurable, default 10 minutes.
  During this grace period Failed ranks ahead of Completed, and both rank
  ahead of Stale — every state that can summon the Board outranks every
  state that cannot (2026-08-02, the Board's attention principle). Waiting
  states do not expire as ordinary Notifications; they remain until the
  runtime reports a new state or the session becomes Stale.
- **Agent Board** — the idle presentation of active Agent Sessions. its
  resting state shows the highest-ranked session richly and represents other
  sessions individually; hover/expand grows it into a screen-bounded,
  scrollable list of every active session. noteworthy Agent Events still use
  the Slot, after which presentation returns to the Agent Board.
- **Rotation Order** — the configured tie-break (v6) among Waiting
  Notifications that share a Priority tier: a ranking over Origin,
  checked before arrival order. it never overrides Priority — a
  higher-Priority arrival still promotes ahead of a lower-Priority one
  regardless of Rotation Order.
- **Promotion** — the moment the highest-priority Waiting Notification
  moves into the Slot. the engine's decision alone; the frontend never
  promotes.
- **Engine** — the one module through which every change to the Slot
  and the Waiting lines flows (plan 037 — landed 2026-07-19 as
  `src-tauri/src/engine.rs`: the queue, the wake, and the live-match
  handle are private to it, so its guarantees are structural, not a
  convention spread across code paths). only the Engine
  Promotes; a change applied through it can never miss a Rotation
  deadline or fail to publish the resulting Slot change to the overlay
  — by construction, not by discipline. accepted Events reach
  Connectors through it, and it enforces the Connector rule that News
  never leaves the machine (see **Connector**).
- **Rotation** — how long a Notification stays Visible, measured from
  Promotion (not from arrival); replaces the old TTL concept (v3.6).
  extended (see **Expanded**) while the Slot is grown. config file keys
  retain `*ttl*` names for file compatibility; the domain term is
  Rotation.
- **Recurring** — a Rotation kind that requeues to the back of its own
  Priority tier's Waiting line after its turn, instead of being
  dropped (v3.6). bounded by supersession or the underlying state
  naturally ending, not a clock. the alternative kind, one-shot, is
  today's plain drop-forever-after-Rotation behaviour.
- **Topic** — the supersession identity carried by a Recurring Event
  (v3.6). a fresh Event sharing a Topic updates the existing
  Notification in place — Waiting or Visible — rather than adding a
  new one; a Visible supersede can grant a small, capped Rotation
  extension if remaining time was already low, but never mutates when
  it was first promoted.
- **Expanded** — a Slot's optional grown state (v3.6; plan 033 made it
  universal, the Silenced work carved out two exceptions): a Medium or
  High Promotion starts Expanded and auto-collapses at half the base
  Rotation window; a Low Promotion and a Breakthrough Promotion start
  compact (the manual expand hotkey still grows either on demand) — the grown first
  half of the turn is display-only and never extends the Rotation. only
  a manual expand (global hotkey) extends the Rotation window, and any
  hotkey press disarms the auto-collapse — a press on an auto-Expanded
  card collapses it.
- **Paused** — engine state in which Promotion is disabled. pushes are
  still accepted and buffered into Waiting (caller is told the app is
  paused); an already-Visible Notification finishes its natural
  Rotation and exits. the Agent Board hides for the duration too — a
  Paused engine quiets the whole notch, not just Promotion. Resuming
  re-enables Promotion immediately;
  nothing is dropped. (v3.6: gates the single Slot, same contract,
  formerly gated a 3-item cap. v5: the tray toggle stays session-only,
  but the persisted `start_paused` config flag — the **Kill Switch** —
  makes the app *launch* Paused.)
- **Silenced** — engine state in which Promotion is priority-gated:
  Medium/Low pushes buffer into Waiting exactly as under Paused, but a
  High Event still promotes (**Breakthrough**). display-layer only —
  Connectors are untouched, pollers keep running, and the overlay's
  idle surface (clock, weather, Agent Board) behaves as normal. ends by
  schedule, timer, or Skip; the backlog then drains under normal
  Rotation. distinct from Paused, which is absolute and sits above
  Silenced — a Paused engine shows nothing, Breakthrough included.
- **Silent Period** — a Silenced span that starts and ends on the
  daily schedule (default 00:00–10:00, one window per day, local wall
  clock). **Skip** — the tray action that ends today's Silent Period
  early; the schedule re-arms at the next window start.
- **Timed Mute** — a Silenced span started manually from the tray with
  a fixed duration (the "meeting mode": preset lengths, auto-resume
  when the timer ends). independent of the Silent Period; overlapping
  silences union — the engine is Silenced until the last one ends.
- **Breakthrough** — the promotion of a High-priority Event while
  Silenced. any High Event qualifies, whatever its Origin; the card
  renders compact (no auto-Expanded opening), and after its Rotation
  ends the engine returns to Silenced.
- **Polling Pause** — historical: a Poller-level state (per source) in
  which the Poller stopped checking its external service; no Events
  were produced and changes during the pause were never seen. distinct
  from Paused: Paused buffers and drops nothing, a Polling Pause
  observed nothing. (v6: no longer tray-toggleable — set once at boot
  from `espn_enabled`/`rss_enabled`; per-source control lives entirely
  in the Settings Window now. 2026-07-18, plan 019: the runtime
  pause/resume gate machinery — unreachable in production since v6
  made this boot-only — was deleted; a source now either spawns at
  boot or doesn't, and there is no live pause/resume or
  re-baseline-on-resume left to describe.)
- **Presentation Mode** — how the window anchors: **Notch** (over the
  macbook's notch cutout) or **HUD** (floating top-center, on
  notchless machines). decided at runtime, never at build time.
- **Settings Window** — the second webview window (v5), opened from
  the tray, where config and secrets are edited. the one window
  allowed to invoke commands into the engine; the overlay never is.
  saving always relaunches the app — there is no hot-reload.
- **notchtap** — the product: the always-on engine + overlay app, and
  the name of the CLI that pushes to it.
- **notchtap-detect** — the standalone swift helper that reports
  screen safe-area geometry so the engine can pick a Presentation
  Mode.
- **Relay** — an external tool forwarding its own notifications into
  notchtap. a Relay is heads-up only: it can never answer back into the
  tool that raised the alert.
- **Connector** — an outbound sink that would receive every accepted
  Event *except News items, which are overlay-only by design* — see
  `IMPLEMENTATION_PLAN.md` §4.6 — and forward the rest off the
  machine, best-effort. a Connector observes acceptance, not
  Promotion: the queue's display rules (cap, Rotation, Paused) never
  apply to it, and its failures never affect the pusher's response.
  telegram shipped as the first (and, so far, only) Connector in v3,
  then was removed 2026-07-27 by operator decision; the generic
  `ConnectorHandle` fan-out framework remains with zero connectors,
  kept for plan 128's Tavily connector.
- **Notifier** — the outbound half of notchtap as a whole: the seam
  through which accepted Events leave the machine. Connectors are its
  members; the overlay is not one. a seam, not a code interface —
  earlier drafts said "the Notifier trait," but no trait exists (and
  none is needed until a second Connector does).
- **Icon Strip** — the row of five neon glyphs (agent, football, music,
  weather, news) that appears inside the hovered right flank. Hidden
  entirely at rest. "Tab" and "icon" name the same thing: **tab** when
  talking about selection state, **icon** when talking about the glyph.
  (plan 171)
- **Tab Selection** — at most one Tab is selected, or none. Selecting
  does not open anything by itself; it decides which source's card the
  existing hover below-block shows the next time the notch is hovered.
  Persists across hovers, and is CLEARED — not remembered — if its
  source stops being live. Rust owns it; the frontend renders it.
  (plan 171)
- **Pull** — reaching for a source deliberately (clicking a Tab, or the
  Prefix keymap) and seeing its current state, with no Promotion and no
  countdown. The orthogonal opposite of the app's original **push**
  model, which is completely unchanged and takes precedence over
  everything pull-related. A pulled card never counts down: it keeps the
  4px floor-strip geometry but repurposes it as a position indicator
  with no drain. (plan 171)
- **Prefix** — the tmux-style keyboard model: one combo (default
  `⌃⇧Space`, configurable) arms a 2-second window, then a single
  follow-up key does exactly one thing and disarms. Additive — the seven
  shipped `⌃⇧` combos keep working prefix-free, forever. (plan 171)
- **News Charge** — the news Tab's two-phase fill model: items landing
  during a poll cycle charge the glyph; a cycle ending with a full batch
  marks it charged; visiting the news Tab clears it. (plan 171)
- **Poller** — an internal event source that repeatedly checks an
  external service (espn in v2) and turns observed *changes* into
  Events. a Poller emits deltas only: the first sighting of a match is
  silent, and repetition of an unchanged fact never produces an Event.
- **Score Update** — an Event produced when a watched match's score
  changes (a goal).
- **Match State** — an Event produced when a watched match's phase
  changes: kickoff, half-time, full-time (and cards, where reported).
