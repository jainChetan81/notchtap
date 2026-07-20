# Plan 076: Surface Telegram connector delivery health in the Settings window

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src-tauri/src/notifier.rs src-tauri/src/settings.rs src-tauri/src/engine.rs src-tauri/src/lib.rs src-tauri/src/status.rs src-tauri/build.rs src-tauri/capabilities/settings.json src/settings/SettingsApp.tsx`
> If any of these changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition. **Review-plan correction
> (2026-07-20): `engine.rs` and `lib.rs` were missing from this list
> entirely** — the original plan cited `engine.rs` line numbers in
> "Current state" but didn't include the file in its own drift check,
> and this review pass adds real new load-bearing citations in both
> files (`Engine::new`'s 6 call sites, the `telegram_connector()` call
> site). `engine.rs` is also the file plans 064 and 072 (same audit
> batch) touch — if either lands first, re-verify this plan's `engine.rs`
> citations before trusting them, don't assume they're still accurate.

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: LOW–MED (new state parallels an already-accepted pattern; the
  main risk is scope creep into a bigger "connector health framework"
  than one connector needs — see Scope)
- **Depends on**: none
- **Category**: direction (this is a grounded feature suggestion, not a
  bug — see `plans/README.md`'s Direction section for how this plan was
  surfaced)
- **Planned at**: commit `f6c2f46`, 2026-07-20
- **Review-plan pass (2026-07-20)**: verified live at HEAD `8743ce6`
  (zero drift on the plan's own drift-check paths). Every code citation
  checked byte-for-byte against the actual files: `notifier.rs`'s
  `send_with_policy`/`RetryDecision::Drop` arm, `engine.rs`'s `accept`
  fan-out, `settings.rs`'s `SecretStatus` struct and
  `send_test_notification`'s location, and the 4-file command-
  registration pattern (confirmed exactly 7 existing entries in all
  three registration files) — all accurate. One citation error found
  and fixed: "Current state" attributed the `update_live_match`/
  `update_weather` precedent to `status.rs:196-229` — it's actually
  `engine.rs:196-229` (confirmed by direct read; plan 073, the source
  this was copied from, cites it correctly as `engine.rs`). More
  substantively, traced the actual data flow the plan's Step 1 only
  gestures at ("thread it through however this codebase's existing
  worker construction passes shared state in") and found a concrete
  answer: `telegram_connector()`'s `ConnectorHandle` return value ends
  up inside `Engine`'s own `connectors: Arc<Vec<ConnectorHandle>>` field
  (`engine.rs:34`), not as separate `tauri`-managed state — and
  `send_test_notification` already establishes the pattern for a
  settings command reaching `Engine` state
  (`engine: tauri::State<'_, Engine>`, `settings.rs:730`). Step 1 and
  Step 2 below are rewritten to point at this concretely (a new `Engine`
  field mirroring `live`/`weather`, not a change to the shared
  `ConnectorHandle` struct) instead of leaving the executor to guess
  between three plausible-looking wrong answers.

## Why this matters

Telegram is this app's **only** outbound connector — per `CONTEXT.md`'s
Connector definition, its entire purpose is best-effort delivery off the
machine while the operator is away. Today, delivery health is
structurally invisible end to end:

- `notifier.rs`'s `send_with_policy` (the retry/drop worker) only logs a
  `tracing::warn!` on a dropped send — nothing else observes it.
- `Engine::accept` fans out to connectors fire-and-forget
  (`connector.offer(&to_offer)` into a bounded channel) — callers get
  `Ok(())` before the worker even attempts the send.
- Boot never validates `bot_token`/`chat_id` against Telegram's API — a
  bad token is discoverable only via a log-file warning (or, per plan
  070 if it lands first, a slightly more discoverable one).
- `send_test_notification`'s Settings-window "test" button
  (`settings.rs:727-741`) returns success once the event is *enqueued*,
  not once Telegram confirms delivery — so the one UI affordance
  operators have for "does Telegram work" doesn't actually verify the
  Telegram leg at all.
- `SecretStatus` (`settings.rs:311-345`) only reports "is a secret
  present," never a delivery/health signal.

A revoked token, wrong `chat_id`, or a run of drops during a live-match's
Recurring updates degrades silently to "my phone just stopped buzzing" —
discoverable only by noticing the absence or manually tailing
`~/Library/Logs/notchtap/notchtap.log`. This finding is unrelated to the
"re-evaluate Telegram only if it proves insufficient" line in
`docs/ARCHITECTURE.md` (that's about adding a *second* connector, a
different axis) — it's about knowing whether the one connector that
exists is actually working.

## Current state

- `src-tauri/src/notifier.rs:212-259` — `send_with_policy`, the
  retry/drop worker loop (already read in full for prior plans — the
  relevant detail is the `RetryDecision::Drop` arm at line ~251-255,
  which currently only logs and returns, with no state update anywhere):

  ```rust
  RetryDecision::Drop => {
      tracing::warn!(?kind, attempt, title = %event.payload.title,
          "telegram send dropped after failure");
      return;
  }
  ```

  And the success path — `send_once` (line 261) returning `Ok(())` inside
  `send_with_policy`'s loop (line 238-239) — also currently does nothing
  beyond `return`.

- `src-tauri/src/engine.rs:159-185` — `Engine::accept`'s fire-and-forget
  fan-out (already excerpted in plan 070's Current state — same
  `connector.offer(&to_offer)` call, no health feedback path).

- `src-tauri/src/settings.rs:311-345` (approximate — re-locate) —
  `SecretStatus`, the existing pattern for a settings-window-readable
  status struct:

  ```rust
  pub struct SecretStatus {
      pub openrouter_api_key: Option<String>,
      pub telegram_bot_token: Option<String>,
      pub telegram_chat_id: Option<String>,
  }
  ```

- `src-tauri/src/engine.rs:196-229`'s `Engine::update_live_match`/
  `update_weather` pattern (already fully excerpted in plan 073 —
  **review-plan correction**: the file is `engine.rs`, not `status.rs`
  as originally written here) — this is the closest existing precedent
  for "a side-channel piece of state, written by a background worker,
  read by the settings/status layer" and should be the model for how
  connector health state gets threaded through, per the audit's own
  fix-sketch recommendation. `engine.rs:35-36` shows the exact field
  shape to mirror: `live: Arc<StdMutex<Option<LiveMatchSummary>>>` /
  `weather: Arc<StdMutex<Option<WeatherSummary>>>` — `std::sync::Mutex`
  aliased as `StdMutex` (`engine.rs:10`), not `tokio::sync::Mutex` (that
  alias is reserved for the queue lock, `engine.rs:13`).

- **The concrete data-flow answer Step 1 needs** (traced by reading
  `lib.rs:171-190` and `engine.rs:34,68,77`): `notifier::telegram_connector()`
  (`notifier.rs:203`) returns `(ConnectorHandle, impl Future<...>)`;
  `lib.rs:175` destructures this, pushes the `ConnectorHandle` into a
  `Vec` that becomes `Engine`'s own `connectors: Arc<Vec<ConnectorHandle>>`
  field (passed into `Engine::new` at construction, `engine.rs:68,77`) —
  it is never separately `app.manage()`'d as its own tauri state.
  Meanwhile `send_test_notification` (`settings.rs:730`) already
  establishes the pattern for a settings command reaching `Engine`
  state: `engine: tauri::State<'_, Engine>` as a parameter. Put
  together: the health state belongs on `Engine` itself, as a new field
  mirroring `live`/`weather` — **not** a new field on the shared
  `ConnectorHandle` struct (that would touch the generic connector
  abstraction every future connector shares, which is exactly the
  "generalized Connector health" scope creep this plan's own Scope
  section says to avoid), and **not** a second, separately-managed
  tauri state (Engine is already the established single home for this
  category of side-channel state).

- The 4-file pattern for adding a new settings-window-only invoke
  command (read all 4 before writing code — this repo enforces a
  deny-by-default ACL and missing any one of these silently breaks it or
  fails to compile):
  1. `src-tauri/src/settings.rs` — the `#[tauri::command]` function itself
  2. `src-tauri/build.rs` — `AppManifest::commands(&[...])` allowlist (currently 7 entries: `get_config`, `get_default_config`, `get_secret_status`, `save_config_and_relaunch`, `set_secret`, `send_test_notification`, `set_appearance`)
  3. `src-tauri/src/lib.rs`'s `generate_handler![...]` list (same 7 entries, mirrored)
  4. `src-tauri/capabilities/settings.json`'s `permissions` array (7 `allow-*` entries, one per command)

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Frontend typecheck | `npx tsc --noEmit` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Frontend lint | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/notifier.rs` — track `last_attempt`/`last_success`/
  `consecutive_failures` in the worker, updated on both the success path
  and the `Drop` arm
- `src-tauri/src/engine.rs` — **review-plan addition, missing from the
  original scope list**: a new field on `Engine` to hold the health
  `Arc` (mirroring `live`/`weather`) and a read accessor for the
  settings command. This also means **every one of `Engine::new`'s 6
  existing call sites needs a new argument added** (mechanical, not a
  restructure) — confirmed by direct grep: `engine.rs:370,438,466,618`
  (test helpers) and `lib.rs:215` (production),`lib.rs:778` (test).
  `Engine::new` is a positional-argument constructor with no
  builder/default pattern, so a new required parameter touches all 6;
  don't let this count read as "bigger than the S–M estimate" when you
  hit it — it's the expected shape of this exact change, not scope
  creep. (`live`/`weather` themselves are NOT passed into `Engine::new`
  — they're always initialized fresh as `Arc::new(StdMutex::new(None))`
  inside the constructor, since nothing outside `Engine` ever writes to
  them. Telegram health is different: the notifier worker, which lives
  outside `Engine` entirely, needs to write into the same `Arc` `Engine`
  reads from — so it must be constructed in `lib.rs` and passed in as a
  parameter, the same way `connectors` already is, not initialized
  internally like `live`/`weather`.)
- `src-tauri/src/lib.rs` — **review-plan addition**: at the
  `telegram_connector()` call site (`lib.rs:175`), construct the health
  `Arc` once, clone it into `WorkerConfig` (for the worker to write —
  requires adding a field to `WorkerConfig`, `notifier.rs:195`) and into
  the new `Engine::new` parameter (for reads).
- A new settings-window invoke command (e.g. `get_connector_health`) —
  touching all 4 files in the pattern above, calling the new `Engine`
  accessor via `engine: tauri::State<'_, Engine>`, the exact parameter
  shape `send_test_notification` already establishes
  (`settings.rs:730`) — do not invent a different way for this command
  to reach shared state.
- `src/settings/SettingsApp.tsx` — one small addition to the Connectors/
  Telegram section rendering the health state

**Out of scope — explicitly, to prevent scope creep into a bigger
"connector health framework" than one connector needs**:
- Boot-time `getMe` validation against Telegram's API — this is a
  separate, slightly larger addition (an actual network call at boot,
  with its own failure-mode questions) than tracking health from
  already-happening sends. If you think it's worth doing, note it as a
  follow-up recommendation in your completion report rather than
  building it here.
- Making `send_test_notification` block on actual Telegram delivery
  confirmation before returning — that changes its latency/UX contract
  (today it returns as soon as the event is enqueued, matching every
  other test-notification button's behavior) and is a separate decision;
  the health state this plan adds is a better fix for "did my test
  notification actually arrive" than blocking the button would be (the
  health state updates shortly after regardless, without changing the
  button's responsiveness).
- Any generalized "Connector trait health" abstraction for a
  hypothetical second connector — this repo's own precedent (`no
  Notifier trait until a second connector exists`, recorded in
  `plans/README.md`'s rejected-findings list) applies here too: build
  this for Telegram concretely, don't design for a connector that
  doesn't exist yet.
- `src-tauri/src/http.rs`, `queue.rs`, `poller.rs` — none need to change.

## Steps

### Step 1: Add health state, owned by `Engine`, written by the worker

In `notifier.rs`, add a small `ConnectorHealth` struct with fields:
`last_attempt: Option<Instant>`, `last_success: Option<Instant>`,
`consecutive_failures: u32`. Use `std::sync::Mutex` (not `tokio::sync::Mutex`
— matching `engine.rs`'s `live`/`weather` precedent, `engine.rs:10,35-36`).
Keep `Instant` internal to this struct; the wire-serializable conversion
(elapsed-since-now, or an epoch-ms timestamp) happens at the DTO boundary
in Step 2, not here — check `status.rs` for how it handles any
similar internal-time-to-wire conversion, if one already exists, before
inventing a new approach.

Update it at the two points in `send_with_policy`:
- On success (`send_once` returns `Ok(())`, `send_with_policy`'s
  `Ok(()) => return` arm): record `last_success = now`,
  `consecutive_failures = 0`.
- On `RetryDecision::Drop`: record `consecutive_failures += 1` (leave
  `last_success` unchanged — it should reflect the last time delivery
  actually worked, not the last attempt).
- Record `last_attempt` on every attempt, success or failure.

Add a `health: Arc<StdMutex<ConnectorHealth>>` field to `WorkerConfig`
(`notifier.rs:195`) so `send_with_policy` can reach it, and add it as a
new parameter to `telegram_connector()` (`notifier.rs:203`) — the
caller (`lib.rs:175`) constructs the `Arc` and passes the same clone
into both `telegram_connector()` and, per below, `Engine::new`.

Add a matching `telegram_health: Arc<StdMutex<ConnectorHealth>>` field
to `Engine` (mirroring `live`/`weather`'s shape) and a new **required**
parameter to `Engine::new` of the same type (`connectors` is the
precedent for a field passed in rather than initialized internally,
since — like health — something outside `Engine` needs to hold a
writable clone; `live`/`weather` are the wrong precedent here since
nothing outside `Engine` ever writes to those). This touches all 6
existing `Engine::new` call sites (`engine.rs:370,438,466,618`,
`lib.rs:215,778`) — add a fresh `Arc::new(StdMutex::new(ConnectorHealth::default()))`
argument at each. This is expected, not scope creep (see "Scope" above).
Add a `pub fn telegram_health(&self) -> ConnectorHealth` accessor to
`Engine` (clones the guarded value out, same shape as any existing
`Engine` read accessor) for Step 2 to call.

At `lib.rs:175`, reorder/adjust so the health `Arc` is constructed
before both `telegram_connector()` and `Engine::new()` are called, and
the same `Arc` clone reaches both. If `config.connectors.telegram.enabled`
is `false` (`lib.rs:171`'s existing gate), `Engine::new` still needs a
value for the new parameter — construct a default/empty
`ConnectorHealth` `Arc` unconditionally in that branch too (the accessor
in Step 2 should read as "no deliveries yet" for a disabled connector,
not error).

**Verify**: `cd src-tauri && cargo build` → exit 0 (this will surface
every `Engine::new` call site that still needs the new argument — fix
all 6, don't silence with `_` or a default that skips real wiring).

### Step 2: Add the `get_connector_health` invoke command

Follow the exact 4-file pattern from "Current state" above:
1. Add `pub async fn get_connector_health(window: tauri::WebviewWindow, engine: tauri::State<'_, Engine>) -> Result<ConnectorHealthDto, String>`
   to `settings.rs`, calling `ensure_settings_window(&window)?` first
   (matching every existing command's first line — `send_test_notification`,
   `settings.rs:727-730`, is the exact parameter-shape precedent to
   copy), calling Step 1's `engine.telegram_health()` accessor, and
   returning a serializable DTO (camelCase via
   `#[serde(rename_all = "camelCase")]`, matching `status.rs`'s wire
   convention — e.g. `lastSuccessMs: Option<i64>`,
   `consecutiveFailures: u32`).
2. Add `"get_connector_health"` to `build.rs`'s `AppManifest::commands(&[...])` list.
3. Add `settings::get_connector_health,` to `lib.rs`'s `generate_handler![...]` list.
4. Add `"allow-get-connector-health"` to `capabilities/settings.json`'s `permissions` array.

**Verify**: `cd src-tauri && cargo build` → exit 0; `grep -c "get_connector_health" src-tauri/build.rs src-tauri/src/lib.rs src-tauri/capabilities/settings.json src-tauri/src/settings.rs` → each file returns at least 1.

### Step 3: Render it in the Settings window

In `SettingsApp.tsx`'s Connectors/Telegram section, add a small
read-only status line calling the new `invoke("get_connector_health")`
(on mount and/or on a light polling interval — check how the existing
`SecretStatus` fetch pattern in this file works and mirror it, since it's
already solving the same "settings-window advisory fetch, isolated from
the critical panel load" problem per `CLAUDE.md`'s own description of
that split). Render something like: "Last delivered: 2 min ago" /
"3 consecutive failures — check your bot token" / "No deliveries yet."

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 4: Tests

- Rust: unit tests in `notifier.rs`'s `mod tests` covering the health
  state transitions (success resets `consecutive_failures` to 0 and
  updates `last_success`; a drop increments `consecutive_failures` and
  leaves `last_success` unchanged) — model after whatever existing
  `send_with_policy`/retry-decision tests already exist in this file's
  `mod tests` (there should be some, given `RetryDecision` is already
  tested).
- Frontend: a test in `SettingsApp.test.tsx` (or wherever the existing
  `SecretStatus`-fetch-on-mount test lives) confirming the new health
  line renders based on a mocked `invoke("get_connector_health")`
  response — mirror that existing test's mock-and-assert shape.

**Verify**: `cd src-tauri && cargo test --locked notifier::` → new tests pass; `npx vitest run` → new test passes.

### Step 5: Full suite + lint, both sides

**Verify**:
- `cd src-tauri && cargo test --locked` → all pass
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo fmt --check` → exit 0
- `npx tsc --noEmit` → exit 0
- `npx vitest run` → all pass
- `npx biome ci .` → exit 0 (or matches pre-existing failure baseline exactly)
- `npx vite build` → exit 0

## Test plan

- Rust: 2-3 new tests in `notifier.rs`'s `mod tests` — success updates
  health correctly, drop increments failure count correctly, repeated
  successes after failures reset the counter.
- Frontend: 1 new test in `SettingsApp.test.tsx` — the health line
  renders from a mocked invoke response, following the existing
  `SecretStatus`-fetch test's mock pattern.
- Verification: `cargo test --locked notifier::` and `npx vitest run` →
  all pass, including new cases.

## Done criteria

- [ ] `cargo test --locked` exits 0, including new `notifier.rs` health tests
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `npx tsc --noEmit` exits 0
- [ ] `npx vitest run` exits 0, including the new `SettingsApp` test
- [ ] `npx biome ci .` exits 0 (or matches pre-existing baseline)
- [ ] `npx vite build` exits 0
- [ ] `get_connector_health` is registered in all 4 required places (build.rs, lib.rs generate_handler, capabilities/settings.json, settings.rs) — this is the exact failure mode CLAUDE.md warns about if any one is missed
- [ ] `capabilities/default.json` (the overlay window's) is byte-identical to before this plan (`git diff src-tauri/capabilities/default.json` empty) — this new command must NEVER reach the overlay window
- [ ] `plans/README.md` status row for 076 updated

## STOP conditions

- **Updated by review-plan pass**: the threading question itself is now
  resolved (see Step 1) — this STOP condition narrows to: if
  `Engine::new` has more or fewer than 6 call sites when you actually
  grep for them, or `telegram_connector()`'s call site at `lib.rs:175`
  has a substantially different shape than described (e.g. the
  `connectors.telegram.enabled` gate has been refactored), re-verify
  against the drift check at the top of this plan before proceeding —
  don't assume the documented shape still holds without checking.
- Any of the 4 command-registration files don't match the excerpts above
  (drift since planning, e.g. an 8th command already added by another
  plan) — re-read the live files and adjust the pattern, but don't skip
  any of the 4 files regardless of how many entries are already there.

## Maintenance notes

- If a second outbound connector is ever added (still explicitly
  deferred per `CONTEXT.md`/`plans/README.md`'s "no Notifier trait"
  precedent), this health-tracking pattern is the natural template to
  generalize at that point — don't generalize it preemptively now.
- The `Instant`-vs-serializable-timestamp question in Step 1 matters:
  check how `status.rs` already solves "internal `Instant`-based state
  needs to reach the JSON wire" (if it has a precedent) before inventing
  a new conversion approach.
