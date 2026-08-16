# notchtap repository guide

Canon guide, every agent + maintainer working this repo.

## writing rules (comments + docs)

Code comments and docs describe the CURRENT product only, present tense. Never write plan numbers, dates, review citations, or "what this used to be" — history lives in git. Keep only: invariants code can't express ("never X, it breaks Y"), cross-file couplings named explicitly, security boundaries, and short API doc comments. One rejected alternative may keep one clause max. Tool directives (biome-ignore, @ts-expect-error, SAFETY:, #[allow] justifications) stay verbatim.

## project state

Shipping v7. Per-plan history lives in `git log` + `plans/INDEX.md` (one line per plan, plus the live backlog). Notes below only things those sources don't tell you.

- `src/` authoritative UI implementation. `prototype/*.html` sole static visual reference: mirrors shipped surfaces, never holds proposals/experiments. `.claude/skills/` single checked-in skill tree (`.agents/skills` symlinks to it); never copy skills between tool-specific dirs.
- test suite must stay green (`cargo test` from `src-tauri/`, `npx vitest run` repo root, ci-gated). current test counts live `docs/TESTING_STRATEGY.md` §0 and only there — don't restate elsewhere.
- **`SlotState::dedup_eq` rule:** continuously-varying wire fields (e.g. `ttl_ms`/`remaining_ms`) must extend `dedup_eq` explicitly, never rely on derived `PartialEq` — deriving makes every tick read as content change.

Dev machine: mac mini (no notch), user `chetanjain`, home `/Users/chetanjain`; rust toolchain installed. Notch-mode behaviour needs per-change verification on the macbook.

## source of truth

This file single physical repo guide; `AGENTS.md` + `CONTEXT.md` compatibility symlinks to it. **domain glossary** at end defines the product's terms; keep code + docs consistent with it. `docs/ARCHITECTURE.md` holds locked decisions (scope, tech stack, cross-device behaviour, queue model, IPC security, agent adapter contract, distribution) — don't re-litigate without user explicitly reopening. `docs/TESTING_STRATEGY.md` holds testing approach: frameworks, per-component coverage, the manual hardware checklist (its §6). Read both before implementation work.

## commands

Standard invocations (`npm run tauri dev`, `npx tsc --noEmit`, `npx vite build`, `cargo build`/`cargo test` from `src-tauri/`, `npx vitest run`) in `package.json` + `justfile` — read those. Non-obvious ones:

- `npx biome check .` local dev command (`npm run lint:fix` auto-applies), but the enforcing gate CI + `just check-web` run is `npx biome ci .` — not interchangeable.
- `./notchtap --title "t" --body "b"` — trigger a notification against the local `/notify` endpoint (default `127.0.0.1:9789`, override `--port` or `$NOTCHTAP_PORT`). committed shell script repo root; `notchtap run -- <cmd>` wraps a long command and pushes a completion card.
- `just test-all` — one-command local verification mirroring `.github/workflows/ci.yml` exactly (see `justfile` for all recipes). fresh clone: `just setup` first. `brew install just` if missing.

No repo-wide coverage percentage gate — the bar is "every listed example surface has a passing test". Physical-hardware behaviour (notch geometry, hud placement, animation look) stays a manual checklist by design (`docs/TESTING_STRATEGY.md` §5/§6).

## architecture

Tauri app: rust core plus react/ts webview ui, not electron, not pure native swift (`docs/ARCHITECTURE.md` §2).

- **rust core** (`src-tauri/`) owns the loopback http listener (`/notify`, `/agent/events`), typed event bus, single-slot priority queue, Agent Registry, window positioning. only process talking to the outside world (cli pushes, pollers, adapter events).
- **react/ts frontend** owns rendering only. two vite entries: overlay (`index.html` → `src/App.tsx`) and settings window (`settings.html` → `src/settings/`), see `vite.config.ts`. per-event-type presentation stays a data change, not a new render path.
- **cross-device behaviour is one runtime branch, not two builds.** the same compiled app runs on the notch macbook + notchless mac mini; a runtime safe-area check picks notch-anchored vs top-center hud. don't fork build targets.
- **swift↔rust boundary is subprocess, not ffi.** the `NSScreen` check lives in a standalone swift cli (`notchtap-detect`) printing json to stdout; rust shells out and parses (`ARCHITECTURE.md` §3). keep the pure decision function separate from the subprocess call — the function is unit-testable, the call isn't.
- **no approve/deny action.** notifications are display-only, auto-dismissed by rotation. don't add a "respond back into the agent cli" loop without reading `ARCHITECTURE.md` §10 — heads-up-only is a locked boundary.

## naming

Project has no association with, doesn't use, any third-party app's branding or code. use the product name (`notchtap`), this repo's name (`mac-notification-nudge`), or generic terms. one narrow exception: supported coding-runtime names (Claude Code, Codex, Kimi, OpenCode) may appear neutrally in adapter identifiers, setup docs, tests/fixtures, UI labels because compatibility requires them. no third-party logos/assets, copied trade dress, or implied affiliation.

## ipc & security

The overlay (`main`) window is **receive-only**: it listens for rust-published events, invokes nothing. `src-tauri/capabilities/default.json` grants listen/unlisten and NOTHING else — no fs, shell, network, emit — and **must never change**.

**The settings window is the one exception, opt-in-gated, not default-safe.** tauri v2 grants app-defined commands to *every* window by default — the settings window's invoke commands are scoped to it alone only because `src-tauri/build.rs` opts into `tauri_build::AppManifest::commands(&[...])` (deny-by-default) plus dedicated `capabilities/settings.json` plus a per-handler label check. **never add a new `#[tauri::command]` without adding it to that `build.rs` list** — else it silently becomes callable from the overlay window, breaking the receive-only guarantee. full contract + command table: `docs/ARCHITECTURE.md` §9.

**Clicks don't imply invoke.** the overlay reacts to icon-strip clicks, but the frontend still never talks to rust: a native `NSEvent` monitor (`src-tauri/src/click.rs`) observes the mouseDown rust-side and pushes a typed `tab-selection-changed` event down the same receive-only channel as `hover-changed`. wanting an `invoke` for a click is the signal you're about to break the boundary, not a gap to fill (`ARCHITECTURE.md` §11).

## rust error handling

- **library/internal modules** (queue, event bus, registry): `thiserror` for structured, matchable variants; tests assert `matches!(err, MyError::QueueFull)`.
- **application boundary** (main.rs, HTTP handlers, CLI entrypoint): `anyhow` for ergonomic propagation. HTTP returns specific status codes (400 malformed, 429 tier full, 500 unexpected) without the internal error type leaking into every signature.

---

## domain glossary

- **Event** — one incoming push (title + body, plus type/priority/rotation). the unit flowing through the system. **Notification** — the display-side view of an Event.
- **Slot** — the single Visible position; never more than one Notification on screen. **Visible** — the Notification occupying it, if any.
- **Waiting** — accepted, not yet shown; three per-Priority lines ordered Rotation Order then arrival, each independently capped (`max_queued_per_tier`).
- **Priority** — `Low | Medium | High`. governs Promotion order + **Preemption**: a strictly-higher-priority arrival cuts the Visible item short; it re-queues at the head of its tier with remaining turn intact. equal never preempts.
- **Origin** — source category: `Football | News | Manual | Agent`. orthogonal to Priority. **Rotation Order** — configured same-tier tie-break ranking over Origin, checked before arrival order; never overrides Priority.
- **Promotion** — the moment the highest-priority Waiting Notification enters the Slot. **Engine** — the one module (`src-tauri/src/engine.rs`) through which every Slot/Waiting change flows; only the Engine promotes, never the frontend; queue, wake, live-match handle private to it, so its guarantees are structural, not convention.
- **Rotation** — how long a Notification stays Visible, measured from Promotion (config keys retain `*ttl*` names for file compatibility). **Recurring** — Rotation kind that requeues to the back of its own tier after its turn instead of dropping (vs one-shot).
- **Topic** — supersession identity on a Recurring Event: a fresh Event sharing it updates the existing Notification in place (Waiting or Visible), with a small capped extension when remaining time is low; never mutates `promoted_at`.
- **Expanded** — the Slot's grown state: Medium/High Promotions start Expanded and auto-collapse at half the base window; Low + Breakthrough start compact; the manual hotkey grows/collapses any card and is the only thing that extends the window.
- **Paused** — Promotion disabled, absolutely. pushes buffer into Waiting, the Visible item finishes naturally, nothing renders (Agent Board included), nothing dropped. tray toggle is session-only; the persisted `start_paused` flag (**Kill Switch**) launches the app Paused.
- **Silenced** — priority-gated quiet: Medium/Low buffer, High still promotes (**Breakthrough**, compact). display-only — pollers and idle surface unaffected. sits below Paused. **Silent Period** — daily scheduled Silenced span (default 00:00–10:00); **Skip** ends today's early. **Timed Mute** — tray-started fixed-duration Silenced span; overlapping silences union.
- **Agent Runtime** — the coding agent behind an Origin-`Agent` Event: `Claude Code | Codex | Kimi | OpenCode`. identifies the producer; not a separate Origin.
- **Agent Adapter** — heads-up-only bridge from one Runtime's lifecycle hooks into notchtap; translates native events into Agent Events, declares its capabilities honestly, never answers on the user's behalf.
- **Agent Event Kind** — runtime-independent meaning: `Permission Requested | Input Required | Completed | Failed | Informational`. **Agent Session State** — `Starting | Working | Waiting For Permission | Waiting For Input | Completed | Failed | Stale`.
- **Agent Session** — one reported session; identity is Runtime + native session id (project path never identity), histories never merged. **Agent Registry** — rust-owned store of live + recently-terminal Sessions; applies Agent Events, enforces ordering (urgency first, FIFO within) and terminal retention; the Agent Board and Agent Notifications both read from it.
- **Adapter Health** — presented status of one adapter (available/partial/unavailable + capabilities + last-accepted-event); reflects whether the adapter delivers, not whether the runtime runs. **Agent Host** — application presenting a Session (T3 Code, terminal, IDE); presentation/focus metadata only, never identity.
- **Agent Board** — idle presentation of Agent Sessions when the Slot is empty: richest for the highest-ranked, hover-expands to a scrollable list. noteworthy Agent Events still use the Slot.
- **Presentation Mode** — **Notch** (over the cutout) or **HUD** (floating top-center); decided at runtime, never build time. **notchtap-detect** — the swift helper reporting safe-area geometry for that decision.
- **Settings Window** — second webview window, opened from the tray; the only window allowed to invoke commands. saving always relaunches — no hot-reload. **notchtap** — the product, and the CLI that pushes to it. **Relay** — an external tool forwarding its notifications in; heads-up only, can never answer back.
- **Icon Strip** — row of source glyphs inside the hovered right flank, hidden entirely at rest. "Tab" (selection state) and "icon" (the glyph) name the same thing. **Tab Selection** — at most one Tab selected, or none; decides which source's card the hover below-block shows. persists across hovers, CLEARED if its source stops being live. rust owns it; the frontend renders it.
- **Pull** — reaching for a source deliberately (Tab click or Prefix key): current state, no Promotion, no countdown. the push model is unchanged and takes precedence over everything pull-related. **News Charge** — the news Tab's fill model: a poll cycle ending with a full batch marks the glyph charged; visiting the news Tab clears it.
- **Prefix** — tmux-style keyboard model: one combo (default `⌃⇧Space`) arms a 2-second window, then a single follow-up key does exactly one thing. additive — the plain `⌃⇧` hotkeys keep working prefix-free.
- **Poller** — internal source repeatedly checking an external service (espn, rss), emitting *deltas* only: first sighting silent, unchanged facts never re-emit. **Score Update** / **Match State** — the football Events (goal; kickoff/half-time/full-time/cards).
