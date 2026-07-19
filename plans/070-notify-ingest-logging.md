# Plan 070: Add tracing to the `/notify` → `Engine::accept` ingest pipeline

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src-tauri/src/http.rs src-tauri/src/engine.rs`
> If either file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: dx
- **Planned at**: commit `f6c2f46`, 2026-07-20

## Why this matters

`docs/ARCHITECTURE.md` §11 states plainly: "this is a background app —
when something breaks, the user needs a log to read. set this up in v1,
not as an afterthought." The write side is built and tested
(`src-tauri/src/logging.rs`, rotating file at
`~/Library/Logs/notchtap/notchtap.log`) — but the actual ingest pipeline
that promise is meant to cover has zero `tracing::` calls anywhere.
`notify_handler` (`src-tauri/src/http.rs:147-223`), the `/notify` HTTP
handler, never logs a rejection reason (bad content-type, malformed
JSON, missing field). `Engine::accept` (`src-tauri/src/engine.rs:159-185`),
which the codebase's own doc-comments call "the one ingest path" for
every source (http `/notify`, Settings test notifications, ESPN/RSS/
weather pollers — per plan 037), never logs acceptance, `QueueFull`
rejection, or connector fan-out. Each `Event` gets a `Uuid::new_v4()` id
(`http.rs:191`) that is never written to the log anywhere in the
codebase.

Practically: if a push to `/notify` silently doesn't show up (wrong
priority tier assumption, a 429 queue-full rejection, a connector drop),
the log file — the one thing ARCHITECTURE.md promises exists for exactly
this scenario — has nothing about it, even at `debug` level (the default
filter in dev builds per `logging.rs:36-40`). The only current
diagnostic path is adding `tracing::` calls, rebuilding, and reproducing.

## Current state

- `src-tauri/src/http.rs:147-223` — `notify_handler`, the full function
  (already read in full for this plan — reproduced here for reference,
  don't re-paste it into the plan, just note the 3 reject points: bad
  content-type at line 156-160, malformed JSON at line 162-163, missing
  title/body at lines 165-171). No `tracing::` import currently exists in
  this file — confirm with `grep -n "^use\|tracing::" src-tauri/src/http.rs`.

- `src-tauri/src/engine.rs:159-185` — `Engine::accept`, the full
  function:

  ```rust
  pub async fn accept(
      &self,
      event: Event,
      bypass_pause_when_slot_empty: bool,
  ) -> Result<(), QueueError> {
      let to_offer = event.clone();
      let now = Instant::now();
      let slot_change = {
          let mut q = self.queue.lock().await;
          if bypass_pause_when_slot_empty {
              q.enqueue_test(event, now)?;
          } else {
              q.enqueue(event, now)?;
          }
          q.slot_state_if_changed()
      };
      self.wake.notify_waiters();
      if let Some(state) = slot_change {
          emit_slot_state(&self.app, state);
      }
      if to_offer.origin != SourceKind::News {
          for connector in self.connectors.iter() {
              connector.offer(&to_offer);
          }
      }
      Ok(())
  }
  ```

  Note `event` is moved into `q.enqueue(...)`/`q.enqueue_test(...)`
  before you'd want to log its id — capture `event.id` (and whatever
  else you want to log) into local variables *before* the `let to_offer
  = event.clone();` line, or log from `to_offer` (the clone) after that
  point, since `to_offer` survives past the move.

- `src-tauri/src/notifier.rs:242,247,252,278` — the existing `tracing::`
  usage pattern in this codebase (the one place tracing already exists,
  on the outbound side) — match this style (structured fields via
  `tracing`'s `key = value` / `key = ?value` / `key = %value` syntax, not
  string interpolation):

  ```rust
  tracing::warn!(?kind, attempt, "telegram send failed — retrying");
  tracing::warn!(attempt, "telegram rejected formatting — resending plain");
  tracing::warn!(?kind, attempt, title = %event.payload.title,
      "telegram send dropped after failure");
  tracing::debug!("telegram request error: {}", e.without_url());
  ```

- `src-tauri/src/logging.rs:35-41` — confirms `debug` is the dev-build
  filter, `info` in release:

  ```rust
  fn log_filter() -> EnvFilter {
      if cfg!(debug_assertions) {
          EnvFilter::new("debug")
      } else {
          EnvFilter::new("info")
      }
  }
  ```

  This means: use `tracing::debug!` for the successful/routine path (so
  it doesn't spam release-build logs) and `tracing::warn!` for rejection
  paths (so those ARE visible in release builds — a silently dropped
  push is exactly the failure mode worth `warn`-level visibility for).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass (this is additive logging, no test count change expected) |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Manual smoke (operator-owed if you can't run the app) | `npm run tauri dev`, then `./notchtap --title t --body b`, then check `~/Library/Logs/notchtap/notchtap.log` | new log lines appear for the accepted push |

## Scope

**In scope**:
- `src-tauri/src/http.rs` (`notify_handler`'s 3 reject branches)
- `src-tauri/src/engine.rs` (`Engine::accept`'s success/QueueFull paths)
- `src-tauri/src/notifier.rs` (thread `event.id` into the existing warn
  logs, so a dropped Telegram send can be tied back to the originating
  push — minimal, matches the existing `title = %event.payload.title`
  pattern already there, just add `id = %event.id`)

**Out of scope**:
- `src-tauri/src/queue.rs` — the audit noted this also has zero tracing,
  but `Engine::accept` is the single choke point every ingest path
  already flows through (per plan 037's design) — logging there gives
  full ingest visibility without needing to instrument `queue.rs`'s
  internals too. Don't add tracing inside `queue.rs` as part of this
  plan; if a future need arises for queue-internal tracing (e.g.
  debugging rotation timing), that's a separate, more invasive plan.
- `src-tauri/src/poller.rs`/`rss_poller.rs`/`weather_poller.rs` — these
  already route through `Engine::accept`, so accept-level logging covers
  their ingest too; don't duplicate logging at the poller call sites.
- Any new request-ID scheme — the `Uuid` already generated per event
  (`http.rs:191`) is sufficient; this plan just needs to actually log it,
  not invent a new identifier.

## Steps

### Step 1: Log `notify_handler`'s 3 reject branches

Add a `tracing::warn!` at each of the 3 early-return error points in
`notify_handler` (`http.rs:147-223`), before the `return Err(...)` /
`?` propagation. Since these happen before an `Event`/`Uuid` exists yet,
log what's available — the rejection reason and, where present, raw
request context (never the raw body — could be large/unbounded):

```rust
if !content_type.starts_with("application/json") {
    tracing::warn!(%content_type, "notify: rejected — content-type must be application/json");
    return Err(HttpError::BadRequest(
        "content-type must be application/json",
    ));
}
```

Repeat for the malformed-JSON and missing-title/missing-body branches,
using `tracing::warn!` with whatever context is available at that point
(e.g. for missing-field, the field name — `title` or `body` — is already
in the `EventError::MissingField("title")` value, so log that).

Add `use tracing;` or the appropriate import if `http.rs` doesn't already
have one (check first — some files in this codebase call
`tracing::warn!` via the fully-qualified path without a `use` statement,
matching `notifier.rs`'s style; follow whichever convention `http.rs`
already uses for other crate-path calls).

**Verify**: `cd src-tauri && cargo build` → exit 0.

### Step 2: Log `Engine::accept`'s success and `QueueFull` paths

In `Engine::accept`, add:
- A `tracing::debug!` right after the successful enqueue (after the
  `q.enqueue(...)`/`q.enqueue_test(...)` call returns `Ok`), logging
  `to_offer.id`, `to_offer.origin`, `to_offer.priority`.
- A `tracing::warn!` on the `QueueFull` path — since `q.enqueue(...)?`
  uses `?` to propagate the error, you'll need to either restructure that
  line to inspect the `Err` case before propagating, or add a `.inspect_err(...)`
  (if the codebase's Rust edition/toolchain supports it — check
  `Cargo.toml`'s `edition`/`rust-version` first) that logs before the `?`
  still propagates the error. Match whichever idiom is already used
  elsewhere in this codebase for "log then propagate" (search for
  `.inspect_err` or a manual `match` pattern in `engine.rs`/`queue.rs`
  first).

Example shape (adjust to match the actual idiom you find):

```rust
let slot_change = {
    let mut q = self.queue.lock().await;
    let enqueue_result = if bypass_pause_when_slot_empty {
        q.enqueue_test(event, now)
    } else {
        q.enqueue(event, now)
    };
    if let Err(ref e) = enqueue_result {
        tracing::warn!(id = %to_offer.id, origin = ?to_offer.origin, error = ?e, "accept: enqueue rejected");
    }
    enqueue_result?;
    tracing::debug!(id = %to_offer.id, origin = ?to_offer.origin, priority = ?to_offer.priority, "accept: enqueued");
    q.slot_state_if_changed()
};
```

**Verify**: `cd src-tauri && cargo build` → exit 0.

### Step 3: Thread `event.id` into `notifier.rs`'s existing drop-log

In `notifier.rs`'s `send_with_policy` (line ~252, the `RetryDecision::Drop`
branch), add `id = %event.id` alongside the existing `title = %event.payload.title`
field:

```rust
RetryDecision::Drop => {
    tracing::warn!(?kind, attempt, id = %event.id, title = %event.payload.title,
        "telegram send dropped after failure");
    return;
}
```

**Verify**: `cd src-tauri && cargo build` → exit 0.

### Step 4: Full suite + lint

**Verify**:
- `cd src-tauri && cargo test --locked` → all pass, same count as
  baseline (no test-count change expected — pure additive logging)
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` →
  exit 0
- `cd src-tauri && cargo fmt --check` → exit 0

### Step 5: Manual smoke check (operator-owed if you can't drive the GUI)

Run `npm run tauri dev`, push a test notification via
`./notchtap --title t --body b`, and confirm
`~/Library/Logs/notchtap/notchtap.log` gains a debug-level "accept:
enqueued" line with the event's id. If you can't run the full app
(headless/CI environment), say so explicitly in your completion report
rather than claiming this step passed.

## Test plan

- No new automated tests — this is observability-only, and this
  codebase's own testing strategy doesn't gate on log content (confirmed
  during the audit: no existing test asserts on `tracing` output). The
  existing `notify_handler`/`Engine::accept` tests already cover the
  behavioral paths (rejection status codes, successful enqueue); adding
  `tracing::` calls alongside them must not change their return values or
  status codes — re-running the existing test suite unchanged is the
  verification.
- Verification: `cargo test --locked` → all pass, same count as baseline.

## Done criteria

- [ ] `cargo test --locked` exits 0, same test count as baseline (no
      new/removed tests expected)
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `grep -c "tracing::" src-tauri/src/http.rs` returns at least 3 (one per reject branch)
- [ ] `grep -c "tracing::" src-tauri/src/engine.rs` returns at least 2 (success + QueueFull paths in `accept`)
- [ ] `grep -n "id = %event.id" src-tauri/src/notifier.rs` shows the threaded id in the drop-log line
- [ ] Manual smoke (Step 5) confirms a log line appears for a real push, or is explicitly flagged operator-owed
- [ ] No files outside `http.rs`/`engine.rs`/`notifier.rs` modified (`git status`)
- [ ] `plans/README.md` status row for 070 updated

## STOP conditions

- The code at any of the three cited locations doesn't match the
  excerpts above (drift since planning) — re-read the live function and
  adjust logging placement accordingly; the intent (log every reject
  reason, log successful accept, thread the id into the drop-log)
  doesn't change even if line numbers do.
- Logging the `QueueFull` case requires restructuring `Engine::accept`'s
  control flow in a way that changes its return type or error semantics
  — if the cleanest fix isn't a small local addition, STOP and report
  rather than doing a larger refactor than this plan's effort estimate
  (S) accounts for.

## Maintenance notes

- Any future ingest path added to this codebase (a 6th `SourceKind`, a
  new HTTP endpoint) should route through `Engine::accept` per plan 037's
  design — which means it inherits this plan's logging for free, as long
  as nobody adds a second bypass path that skips `accept`.
- If log volume from `debug`-level "accept: enqueued" lines ever becomes
  a concern (e.g. a high-frequency source), that's a signal for a
  targeted downgrade to a lower-cardinality log (e.g. counter metrics
  instead of per-event lines) — not a reason to remove the logging this
  plan adds.
