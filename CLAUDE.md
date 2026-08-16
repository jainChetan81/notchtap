# notchtap repository guide

Canon guide, every agent + maintainer working this repo.

## writing rules (comments + docs)

Code comments and docs describe the CURRENT product only, present tense. Never write plan numbers, dates, review citations, or "what this used to be" — history lives in git. Keep only: invariants code can't express ("never X, it breaks Y"), cross-file couplings named explicitly, security boundaries, and short API doc comments. One rejected alternative may keep one clause max. Tool directives (biome-ignore, @ts-expect-error, SAFETY:, #[allow] justifications) stay verbatim.

## project state

Scaffolded, shipping v6. Per-plan history NOT duplicated here — read `plans/done/` (one file per plan) + `git log` for what landed when. Notes below only things those sources don't tell you.

- docs folder not part of app build; tauri/rust/web project lives repo root alongside `docs/`.
- `src/` authoritative UI implementation. `prototype/*.html` sole static visual reference: mirrors shipped surfaces, not wired into build, must never hold proposals/experiments.
- `.claude/skills/` single checked-in skill tree; `.agents/skills` tracked compatibility symlink to it. never copy skills between tool-specific dirs. machine-local AI settings, credentials, caches, worktrees stay ignored.
- **repo cleanup, 2026-08-03** — removed, not lost. everything below in `git log`, restorable via `git checkout <commit> -- <path>` if wanted again; nothing here consumed by app build, so removal safe:
  - `prototypes/` (plural) — proposal/scratch mocks, incl two r3 tab-notch design mocks. those two also still on branch `feat/tab-notch-redesign`. tab feature shipped, so `src/` authoritative over them now; spec citing them (`docs/superpowers/specs/2026-08-02-tab-notch-design.md`) says so.
  - `DESIGN.html` — old system-level token/law reference. predated Agent Board, gone stale; `prototype/index.html` + `vendor/shared-ui/design/tokens.css` live equivalents now.
  - `.mcp.json` + `mcp-servers/` — project-local MCP config + unused research server. PAL server this repo actually uses configured at USER level (`~/.claude.json`), unaffected. `.mcp.json` now gitignored, local one recreatable without landing in commit.
  - `skills-lock.json`, `assets/branding/build_branding.py`, unreferenced weather glyphs, ios/android/windows icon sets tauri never bundles (`tauri.conf.json` names only five mac/win icons remaining).
- test suite must stay green (`cargo test` from `src-tauri/`, `npx vitest run` repo root, all gated by ci). current test counts live `docs/TESTING_STRATEGY.md` §0 and only there — don't restate elsewhere.
- **`SlotState::dedup_eq` rule:** continuously-varying wire fields (e.g. `ttl_ms`/`remaining_ms`) must extend `dedup_eq` explicitly, must never rely on derived `PartialEq` — deriving makes every tick read as content change.
- hover primitive shipped (tracking area, rust-derived card rect, `hover-changed` event); all four hover CONSUMER features shipped plan 093: TTL-bar hover-pause, idle weather peek, scorecard reveal-on-hover, idle expand-on-hover.
- frontend toolchain spike (TypeScript 7 / Vite 8 / Vitest 4) returned **GO** verdict, but nothing adopted — `package.json` untouched, adoption separate unwritten plan. don't read GO as "already done".
- remaining open work: manual checklist rows `docs/IMPLEMENTATION_PLAN.md` §6, plus whatever `plans/` holds.

<!-- trimmed 2026-07-21: a plan-by-plan changelog (v1–v6, plans
037–087) lived here. it was reconstructible from plans/done/ and git
log, so it was cut to keep this file cheap to load every session. -->

`docs/archive/BLIND_REVIEW.md` + `docs/archive/CHANGES_SUMMARY.md` were changelog/audit artifacts from planning pass, not source of truth — decisions they describe already folded into three docs below. `docs/archive/V1_TECHNICAL_SPEC.md`, `docs/archive/V2_TECHNICAL_SPEC.md`, `docs/archive/V3_TECHNICAL_SPEC.md` likewise archived: those phases shipped. all five removed repo close-out (2026-07-23), retrievable via `git log -- docs/archive/`. `docs/V3_6_TECHNICAL_SPEC.md`, `docs/V5_TECHNICAL_SPEC.md`, `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` active working-draft specs now.

Dev machine: mac mini (no notch), user `chetanjain`, home `/Users/chetanjain`; rust toolchain installed. Notch-mode behaviour still needs per-change verification on macbook.

## source of truth

This file single physical repo guide. `AGENTS.md` + `CONTEXT.md` compatibility symlinks to it for tools discovering those conventional names. **domain glossary** at end defines terms like Promotion, Visible/Waiting, Paused, Presentation Mode; keep code + docs consistent with it.

`docs/ARCHITECTURE.md` holds locked decisions (scope phasing, tech stack, cross-device behaviour, distribution model) — don't re-litigate without user explicitly reopening. `docs/IMPLEMENTATION_PLAN.md` holds phased build sequence + exit criteria through v7. `docs/TESTING_STRATEGY.md` holds testing approach — frameworks, what's tdd'd first vs written after, per-component test plan, what's deliberately manual-only verification. Read all three before starting implementation work.

`docs/V3_6_TECHNICAL_SPEC.md`, `docs/V5_TECHNICAL_SPEC.md`, `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` are v0 drafts operationalizing those three into code-level specifics for currently-active phases — exact file layout, struct/type shapes, `/notify` json schema, `notchtap-detect` subprocess contract, config/logging paths, error-to-status-code mapping. unlike `ARCHITECTURE.md`, neither locked — adjust freely as implementation surfaces friction. if a change there actually a *decision* change (default, scope boundary), make edit in `ARCHITECTURE.md` instead. equivalent v1/v2/v3 specs archived at `docs/archive/` — those phases already shipped, historical records not active contracts (same status as `BLIND_REVIEW.md`/`CHANGES_SUMMARY.md` above). all five removed repo close-out (2026-07-23); retrievable via `git log -- docs/archive/`.

## commands (once scaffolded)

Standard invocations (`npm run tauri dev`, `npx tsc --noEmit`, `npx vite build`, `cargo build`/`cargo test` from `src-tauri/`, `npx vitest run`) in `package.json` + `justfile` — read those. Non-obvious ones:

- `npx biome check .` local dev command (`npm run lint:fix` auto-applies), but enforcing gate CI + `just check-web` run is `npx biome ci .` — not interchangeable.
- `./notchtap --title "t" --body "b"` — manually trigger notification against local `/notify` endpoint (default `127.0.0.1:9789`, override via `--port` or `$NOTCHTAP_PORT`), for testing queue/animation without real event source. cli committed shell script repo root; besides flags also has `run` subcommand (plan 058) — `notchtap run -- pnpm build` wraps long-running command, pushes completion card when finishes (skipped for successful runs under `--min-secs`, default 15s; failure always pushes).
- `just test-all` — one-command local verification mirroring `.github/workflows/ci.yml` exactly (see `justfile` repo root for full recipe list: `setup`, `dev`, `test-rust`, `check-rust`, `test-web`, `check-web`, `audit-web`, `build-web`, `check-cli`, `check-swift`, `build-media-adapter` — that last compiles vendored MediaRemote framework, installs under `~/Library/Application Support/notchtap/`, no sudo). fresh clone: run `just setup` (`npm ci`) first — `test-all` doesn't install web deps for you. `just push "title" "body"` wraps `./notchtap` cli call above. `just` not installed on dev machine yet — `brew install just` first.

`cargo test` + `npx vitest run` both should be clean before any phase in `docs/IMPLEMENTATION_PLAN.md` marked done — see that doc's §6 + `docs/TESTING_STRATEGY.md` §7. no repo-wide coverage percentage gate (see `docs/TESTING_STRATEGY.md` §6 for why) — bar is "every example case listed for phase's components has passing test," not coverage number. physical-hardware behaviour (notch geometry, hud placement, animation look) stays manual checklist by design — `docs/TESTING_STRATEGY.md` §5 explains why those specific things not worth automating.

## architecture (once scaffolded)

Tauri app: rust core plus react/ts webview ui, not electron, not pure native swift (see `docs/ARCHITECTURE.md` §8 for why).

- **rust core** (`src-tauri/src/main.rs`) owns local http listener on `127.0.0.1:9789` (`/notify`), typed event bus, fifo notification queue (capped concurrent visible items, per-item ttl), window positioning. only process talking to outside world (cli pushes, internal pollers, v7's loopback Agent Adapter events).
- **react/ts frontend** owns rendering only. two vite entries: overlay (`index.html` → `src/App.tsx`, `src/styles.css`) and settings window (`settings.html` → `src/settings/`), see `vite.config.ts`. overlay receives queued events via tauri's event system, renders through animation template. v1 has exactly one template (enter/hold/exit); v2 replaces this with config table keyed by event type — should stay data change, not new render path.
- **cross-device behaviour single runtime branch, not two builds.** same compiled app runs on both notch macbook + notchless mac mini; runtime check (`NSScreen.main?.safeAreaInsets.top > 0`) decides whether window anchors over notch cutout or floats as top-center hud. don't fork into separate build targets.
- **swift↔rust boundary is subprocess, not ffi.** `NSScreen` check lives in standalone swift cli (`notchtap-detect`) printing json to stdout; rust core shells out via `std::process::Command`, parses result (`docs/ARCHITECTURE.md` §5). keep pure decision logic (`fn presentation_mode
  (safe_area_top_inset: f64) -> Mode`) separate from that subprocess call — function unit-testable, subprocess call not (`docs/TESTING_STRATEGY.md` §4.4).
- **v1 has no approve/deny action.** notifications display-only, auto-dismissed by ttl. don't add "respond back into agent cli" loop without reading `docs/ARCHITECTURE.md` §20 first — needs agent cli's own permission/pre-tool hooks, deliberately separate harder problem, out of scope until explicitly requested.

## naming

Project no association with, doesn't use, any third-party app's branding or code. use product name (`notchtap`), this repo's own name (`mac-notification-nudge`), or generic terms by default. v7's one narrow exception: supported coding-runtime names (Claude Code, Codex, Kimi, OpenCode) may appear neutrally in adapter identifiers, setup docs, tests/fixtures, UI labels because compatibility requires them. no third-party logos/assets, copied trade dress, or implied affiliation.

## ipc & security (once scaffolded)

Tauri v2 uses capabilities/permissions system. frontend in this app **receive-only** in v1 — listens for single custom event from rust core (`notification-promoted`, emitted exactly once per item at promotion time, never at enqueue), renders it. no frontend-to-rust invoke commands in v1.

`src-tauri/capabilities/default.json` should be locked down to minimum: one permission for custom event channel, no file-system access, no shell access, no network access from frontend. frontend shouldn't be able to trigger notifications — only display what rust core sends it.

**v5 settings window is one exception, and it's opt-in-gated, not default-safe.** tauri v2 grants app-defined commands to *every* window by default — settings window's seventeen invoke commands (`clear_history`, `clear_queue`, `get_about_info`, `get_agent_health`, `get_config`, `get_default_config`, `get_history`, `get_queue`, `get_recent_log_lines`, `get_secret_status`, `save_config_and_relaunch`, `search_news_now`, `send_agent_test_event`, `set_secret`, `send_test_notification`, `set_appearance`, `skip_current`) scoped to it alone only because `src-tauri/build.rs` opts into `tauri_build::AppManifest::commands(&[...])` (deny-by-default) plus dedicated `capabilities/settings.json`. never add new `#[tauri::command]` without adding it to that `build.rs` list — else it silently becomes callable from overlay (`main`) window too, breaking receive-only guarantee above. `capabilities/default.json` must never change. full contract: `docs/V5_TECHNICAL_SPEC.md` §2.

**plan 171 added CLICK path without touching any of above — read this before assuming click implies invoke.** overlay reacts to clicks on icon strip, but frontend still never talks to rust: native `NSEvent` local monitor (`src-tauri/src/click.rs`) observes mouseDown on rust side, decides which icon it hit, pushes typed `tab-selection-changed` event down same receive-only channel `hover-changed` already uses. not stylistic preference — overlay's capability file grants event listen/unlisten and NOTHING else, so click webview sees has no way to tell rust about it. if you ever find yourself wanting an `invoke` for a click, that's signal you're about to break boundary, not gap to fill. relatedly, `set_ignore_cursor_events` no longer unconditionally `true` — see `docs/ARCHITECTURE.md` §22 for exactly when it opens, why hit-test not toggle narrows clicks to strip.

## rust error handling

- **library/internal modules** (queue, event bus, event types): use `thiserror` for structured, matchable error variants. tests should assert `matches!(err, MyError::QueueFull)`.
- **application boundary** (main.rs, HTTP handlers, CLI entrypoint): use `anyhow` for ergonomic error propagation. HTTP layer returns specific status codes (400 malformed json, 429 queue full, 500 unexpected), but internal error type needn't leak into every function signature.

Split standard in rust ecosystem, matches testing strategy: unit tests match on `thiserror` variants; integration tests assert on HTTP status codes.

---

## domain glossary

- **Event** — one incoming push (title + body, plus type/priority/rotation assigned by engine). unit flowing through system.
- **Notification** — Event being (or waiting to be) displayed. every Notification is an Event; "Notification" word for display-side view of it.
- **Slot** — single Visible position (v3.6; replaces old 3-item "Visible... ordered as a stack" model). never more than one Notification on screen at a time.
- **Visible** — Notification currently occupying Slot, if any (at most one, see **Slot**).
- **Waiting** — Notifications accepted but not yet shown, ordered **within their own Priority tier** by Rotation Order first, arrival order (FIFO) as tie-break (v3.6: Low/Medium/High three separate lines, not one; v6 added Rotation Order ahead of pure FIFO). capped per tier (`max_queued_per_tier`); pushes beyond a tier's own cap rejected, independent of other two tiers.
- **Priority** — `Low | Medium | High` on every Event (v3.6), independent of `EventType` — not every high-priority thing a score. governs Promotion order + **Preemption**: higher-priority Waiting items promoted next, strictly-higher-priority arrival cuts currently-Visible item short — preempted card re-queues at head of its own tier with remaining turn intact, shows again once every higher-priority card finished. equal priority never preempts; waits its turn. (pre-silence contract was no-interruption ever; rewritten with Silenced work, 2026-07-27.)
- **Origin** — which source category produced an Event (v6): `Football |
  News | Manual | Agent | Weather`. orthogonal to Priority + `EventType` — source's Origin never changes, but its Priority user-configurable per source. only thing Origin governs is Rotation Order.
- **Agent Runtime** — coding agent that produced Event whose Origin is **Agent**, initially `Claude Code | Codex | Kimi | OpenCode`. Runtime identifies producer for presentation + runtime-specific policy; doesn't create separate Origin or Rotation Order category.
- **Agent Adapter** — heads-up-only bridge from one Agent Runtime's lifecycle hooks into notchtap. translates runtime's native event into Agent Event; notchtap doesn't launch, supervise, or scrape runtime, adapter never answers on user's behalf.
- **Agent Adapter Capability** — one kind of structured info/behavior an Agent Adapter can provide: completion, failure, permission requests, progress, tool details, subagents, or opening originating session. adapters declare capabilities; partial support explicit, unsupported info omitted rather than invented.
- **Agent Event Kind** — runtime-independent meaning assigned by Agent Adapter: `Permission Requested | Input Required | Completed |
  Failed | Informational`. runtime-native event names remain diagnostic detail, not presentation branches.
- **Agent Session** — one active coding-agent session reported by Agent Adapter. continuously updated state can appear on idle surface without producing Notification for every progress tick; only noteworthy Agent Events enter Slot. every session independent identity + history, even when multiple sessions share Agent Runtime or project; histories never merged. identity is Agent Runtime plus native session identifier; adapter may provide degraded process/start-time fallback, but project path alone never a session identity.
- **Agent Session State** — runtime-independent lifecycle of Agent Session: `Starting | Working | Waiting For Permission | Waiting For Input |
  Completed | Failed | Stale`. adapters translate native lifecycle names into these states. Informational an Agent Event Kind, not Session State; Stale means reporting ended without clean terminal event.
- **Agent Registry** — in-process store of every live + recently terminal Agent Session, keyed by Agent Runtime plus native session identity. applies Agent Events to advance Session State, enforces Agent Session Order + Terminal Retention, source Agent Board + Origin::Agent Notifications both read from; updates independently of whether corresponding Notification can enter Slot.
- **Adapter Health** — presented status of one Agent Adapter: available, partial, or unavailable, alongside declared Agent Adapter Capabilities, last-accepted-event time, any compatibility note. reflects whether adapter actually delivering events, not whether underlying Agent Runtime happens to be running.
- **Agent Host** — application presenting Agent Session, such as T3 Code, terminal, or IDE. Host optional presentation + open/focus metadata; not part of Agent Session identity or Origin.
- **Agent Session Order** — urgency first, then arrival order among sessions at equal urgency. session needing input/permission ranks ahead of passive Working session; equal-urgency sessions remain FIFO.
- **Terminal Retention** — how long terminal Agent Session remains on Agent Board before moving to history: configurable, default 10 minutes. During grace period Failed ranks ahead of Completed, both rank ahead of Stale — every state that can summon Board outranks every state that can't (2026-08-02, Board's attention principle). Waiting states don't expire as ordinary Notifications; remain until runtime reports new state or session becomes Stale.
- **Agent Board** — idle presentation of active Agent Sessions. resting state shows highest-ranked session richly, represents other sessions individually; hover/expand grows it into screen-bounded, scrollable list of every active session. noteworthy Agent Events still use Slot, after which presentation returns to Agent Board.
- **Rotation Order** — configured tie-break (v6) among Waiting Notifications sharing Priority tier: ranking over Origin, checked before arrival order. never overrides Priority — higher-Priority arrival still promotes ahead of lower-Priority one regardless of Rotation Order.
- **Promotion** — moment highest-priority Waiting Notification moves into Slot. engine's decision alone; frontend never promotes.
- **Engine** — one module through which every change to Slot + Waiting lines flows (plan 037 — landed 2026-07-19 as `src-tauri/src/engine.rs`: queue, wake, live-match handle private to it, so its guarantees structural, not convention spread across code paths). only Engine Promotes; change applied through it can never miss Rotation deadline or fail to publish resulting Slot change to overlay — by construction, not discipline. accepted Events reach Connectors through it, enforces Connector rule that News never leaves the machine (see **Connector**).
- **Rotation** — how long Notification stays Visible, measured from Promotion (not from arrival); replaces old TTL concept (v3.6). extended (see **Expanded**) while Slot grown. config file keys retain `*ttl*` names for file compatibility; domain term is Rotation.
- **Recurring** — Rotation kind that requeues to back of its own Priority tier's Waiting line after its turn, instead of being dropped (v3.6). bounded by supersession or underlying state naturally ending, not a clock. alternative kind, one-shot, today's plain drop-forever-after-Rotation behaviour.
- **Topic** — supersession identity carried by Recurring Event (v3.6). fresh Event sharing Topic updates existing Notification in place — Waiting or Visible — rather than adding new one; Visible supersede can grant small, capped Rotation extension if remaining time already low, but never mutates when it was first promoted.
- **Expanded** — Slot's optional grown state (v3.6; plan 033 made it universal, Silenced work carved out two exceptions): Medium or High Promotion starts Expanded, auto-collapses at half base Rotation window; Low Promotion + Breakthrough Promotion start compact (manual expand hotkey still grows either on demand) — grown first half of turn display-only, never extends Rotation. only manual expand (global hotkey) extends Rotation window, any hotkey press disarms auto-collapse — press on auto-Expanded card collapses it.
- **Paused** — engine state where Promotion disabled. pushes still accepted, buffered into Waiting (caller told app paused); already-Visible Notification finishes natural Rotation, exits. Agent Board hides for duration too — Paused engine quiets whole notch, not just Promotion. Resuming re-enables Promotion immediately; nothing dropped. (v3.6: gates single Slot, same contract, formerly gated 3-item cap. v5: tray toggle stays session-only, but persisted `start_paused` config flag — the **Kill Switch** — makes app *launch* Paused.)
- **Silenced** — engine state where Promotion priority-gated: Medium/Low pushes buffer into Waiting exactly as under Paused, but High Event still promotes (**Breakthrough**). display-layer only — Connectors untouched, pollers keep running, overlay's idle surface (clock, weather, Agent Board) behaves normal. ends by schedule, timer, or Skip; backlog then drains under normal Rotation. distinct from Paused, which absolute and sits above Silenced — Paused engine shows nothing, Breakthrough included.
- **Silent Period** — Silenced span starting/ending on daily schedule (default 00:00–10:00, one window per day, local wall clock). **Skip** — tray action ending today's Silent Period early; schedule re-arms at next window start.
- **Timed Mute** — Silenced span started manually from tray with fixed duration (the "meeting mode": preset lengths, auto-resume when timer ends). independent of Silent Period; overlapping silences union — engine Silenced until last one ends.
- **Breakthrough** — promotion of High-priority Event while Silenced. any High Event qualifies, whatever its Origin; card renders compact (no auto-Expanded opening), after its Rotation ends engine returns to Silenced.
- **Polling Pause** — historical: Poller-level state (per source) where Poller stopped checking external service; no Events produced, changes during pause never seen. distinct from Paused: Paused buffers + drops nothing, Polling Pause observed nothing. (v6: no longer tray-toggleable — set once at boot from `espn_enabled`/`rss_enabled`; per-source control lives entirely in Settings Window now. 2026-07-18, plan 019: runtime pause/resume gate machinery — unreachable in production since v6 made this boot-only — deleted; source now either spawns at boot or doesn't, no live pause/resume or re-baseline-on-resume left to describe.)
- **Presentation Mode** — how window anchors: **Notch** (over macbook's notch cutout) or **HUD** (floating top-center, notchless machines). decided at runtime, never build time.
- **Settings Window** — second webview window (v5), opened from tray, where config + secrets edited. one window allowed to invoke commands into engine; overlay never is. saving always relaunches app — no hot-reload.
- **notchtap** — the product: always-on engine + overlay app, and name of CLI that pushes to it.
- **notchtap-detect** — standalone swift helper reporting screen safe-area geometry so engine can pick Presentation Mode.
- **Relay** — external tool forwarding its own notifications into notchtap. Relay heads-up only: can never answer back into tool that raised alert.
- **Connector** — outbound sink that would receive every accepted Event *except News items, which are overlay-only by design* — see `IMPLEMENTATION_PLAN.md` §4.6 — forward rest off machine, best-effort. Connector observes acceptance, not Promotion: queue's display rules (cap, Rotation, Paused) never apply to it, its failures never affect pusher's response. telegram shipped as first (and, so far, only) Connector in v3, then removed 2026-07-27 by operator decision; generic `ConnectorHandle` fan-out framework remains with zero connectors, kept for plan 128's Tavily connector.
- **Notifier** — outbound half of notchtap as whole: seam through which accepted Events leave machine. Connectors its members; overlay not one. a seam, not a code interface — earlier drafts said "the Notifier trait," but no trait exists (none needed until second Connector does).
- **Icon Strip** — row of five neon glyphs (agent, football, music, weather, news) appearing inside hovered right flank. Hidden entirely at rest. "Tab" and "icon" name same thing: **tab** talking about selection state, **icon** talking about the glyph. (plan 171)
- **Tab Selection** — at most one Tab selected, or none. Selecting doesn't open anything by itself; decides which source's card existing hover below-block shows next time notch hovered. Persists across hovers, CLEARED — not remembered — if its source stops being live. Rust owns it; frontend renders it. (plan 171)
- **Pull** — reaching for a source deliberately (clicking a Tab, or Prefix keymap), seeing its current state, no Promotion, no countdown. orthogonal opposite of app's original **push** model, which completely unchanged, takes precedence over everything pull-related. pulled card never counts down: keeps 4px floor-strip geometry but repurposes as position indicator with no drain. (plan 171)
- **Prefix** — tmux-style keyboard model: one combo (default `⌃⇧Space`, configurable) arms 2-second window, then single follow-up key does exactly one thing, disarms. Additive — seven shipped `⌃⇧` combos keep working prefix-free, forever. (plan 171)
- **News Charge** — news Tab's two-phase fill model: items landing during poll cycle charge glyph; cycle ending with full batch marks it charged; visiting news Tab clears it. (plan 171)
- **Poller** — internal event source repeatedly checking external service (espn in v2), turns observed *changes* into Events. Poller emits deltas only: first sighting of match silent, repetition of unchanged fact never produces Event.
- **Score Update** — Event produced when watched match's score changes (a goal).
- **Match State** — Event produced when watched match's phase changes: kickoff, half-time, full-time (and cards, where reported).