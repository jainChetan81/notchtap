# Plan 070: Add tracing to the `/notify` → `Engine::accept` ingest pipeline

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src-tauri/src/http.rs src-tauri/src/engine.rs`
> **`engine.rs` WILL show a diff** — plan 076 (Telegram connector health)
> already landed and added a `telegram_health` field/accessor before
> `Engine::accept`, shifting it from the original planning-time lines
> 159-185 to its current 174-200 (content otherwise byte-identical — this
> is cited correctly below already). That specific shift is expected, not
> a STOP condition. `http.rs` should show no diff; if it does, or if
> `engine.rs` differs by more than a pure line shift, re-read the live
> file before proceeding.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: dx
- **Planned at**: commit `f6c2f46`, 2026-07-20
- **Review-plan pass (2026-07-20)**: own read + a required fresh-context
  subagent cold-read (authored in-session). Found and fixed: (1) a false
  claim — "notifier.rs [is] the one place tracing already exists" — a
  repo-wide grep shows `tracing::` calls in 10 files, not one (event.rs,
  lib.rs, login_item.rs, notifier.rs, poller.rs, presentation.rs,
  rss_poller.rs, settings.rs, status.rs, weather_poller.rs); corrected
  below to just cite notifier.rs as *a* representative example of the
  structured-field style to match, not the sole occurrence. (2) Step 2
  sent the executor hunting for a "log then propagate" idiom
  (`.inspect_err` or similar) that doesn't exist anywhere in this
  codebase — confirmed via `grep -rn "inspect_err" src-tauri/src/*.rs`,
  zero hits. Removed the false research pointer; the plan's own sketch
  is the pattern to use directly, no further precedent to find. (3)
  `engine.rs` has drifted (see the drift-check note above, plan 076
  landed) — `Engine::accept` is now at lines 174-200, not 159-185;
  content unchanged, just shifted. (4) `notifier.rs`'s four quoted
  tracing calls had drifted further than a citation should
  (`242/247/252/278` → real current locations `271/276/281/310`) —
  fixed to the verified-current lines.
- **Second review-plan pass (2026-07-20, same day)**: re-invoked after
  more concurrent landings; re-verified all citations live. Steps 1
  (`http.rs:147-223`) and 2 (`engine.rs:174-200`, `Engine::accept`) are
  still byte-identical to what's already cited — no further drift there.
  But Step 3's target code had gone stale in a genuinely dangerous way:
  plan 076 (already landed) inserted a `cfg.health.lock().unwrap()
  .consecutive_failures += 1;` line (plus a comment) directly between
  the `tracing::warn!` call and `return;` in the exact
  `RetryDecision::Drop` branch Step 3 edits. The plan's shown
  "before"/"after" snippets only had 2 statements (warn + return); an
  executor doing a literal block-replace against the plan's stale
  snippet would have silently deleted plan 076's health-tracking
  increment while adding the `id` field. Fixed Step 3 to show the real
  3-statement current shape, instruct changing only the `tracing::warn!`
  line, and added a grep-based verification that the health-tracking
  line survived the edit.

## Why this matters

`docs/ARCHITECTURE.md` §11 states plainly: "this is a background app —
when something breaks, the user needs a log to read. set this up in v1,
not as an afterthought." The write side is built and tested
(`src-tauri/src/logging.rs`, rotating file at
`~/Library/Logs/notchtap/notchtap.log`) — but the actual ingest pipeline
that promise is meant to cover has zero `tracing::` calls anywhere.
`notify_handler` (`src-tauri/src/http.rs:147-223`), the `/notify` HTTP
handler, never logs a rejection reason (bad content-type, malformed
JSON, missing field). `Engine::accept` (`src-tauri/src/engine.rs:174-200`),
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

- `src-tauri/src/engine.rs:174-200` — `Engine::accept`, the full
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

- `src-tauri/src/notifier.rs:271,276,281,310` — a representative sample
  of this codebase's `tracing::` structured-field style (tracing is
  actually used in 10 files already, not just this one — but notifier.rs
  is the clearest example to match): use `key = value` / `key = ?value`
  / `key = %value` syntax, not string interpolation:

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
  uses `?` to propagate the error, restructure that line to inspect the
  `Err` case before propagating. There is no existing "log then
  propagate" idiom elsewhere in this codebase to match (confirmed: zero
  `.inspect_err` usage anywhere in `src-tauri/src/`) — use the shape
  below directly, it's the pattern for this plan, not a precedent lookup.

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

In `notifier.rs`'s `send_with_policy` (line 280, the `RetryDecision::Drop`
branch), add `id = %event.id` alongside the existing `title = %event.payload.title`
field — **only on the `tracing::warn!` line**. Plan 076 (Telegram
connector health, already landed) added a `consecutive_failures`
increment right after this warn call, in the same branch — the real
current shape is:

```rust
RetryDecision::Drop => {
    tracing::warn!(?kind, attempt, title = %event.payload.title,
        "telegram send dropped after failure");
    // a drop is a failed delivery: bump the counter but
    // leave last_success at the last time it actually worked
    cfg.health.lock().unwrap().consecutive_failures += 1;
    return;
}
```

Change only the `tracing::warn!` line; leave the comment and the
`cfg.health...` line untouched:

```rust
RetryDecision::Drop => {
    tracing::warn!(?kind, attempt, id = %event.id, title = %event.payload.title,
        "telegram send dropped after failure");
    // a drop is a failed delivery: bump the counter but
    // leave last_success at the last time it actually worked
    cfg.health.lock().unwrap().consecutive_failures += 1;
    return;
}
```

**Verify**: `cd src-tauri && cargo build` → exit 0; `grep -n "consecutive_failures += 1" src-tauri/src/notifier.rs` still shows exactly one match — confirms this step didn't accidentally delete plan 076's health-tracking line while editing the same branch.

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
- [ ] No *source* files outside `http.rs`/`engine.rs`/`notifier.rs` modified (`git status` — `plans/README.md` is expected to change too; everything else is out of scope)
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
