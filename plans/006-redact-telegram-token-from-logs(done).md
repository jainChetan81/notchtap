# Plan 006: Prevent telegram transport errors from logging the bot token

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in "STOP conditions" occurs, stop and report; do not
> improvise. When done, update this plan's status row in `plans/README.md`
> unless a reviewer told you they maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat b1981c9..HEAD -- src-tauri/src/notifier.rs docs/TESTING_STRATEGY.md plans/README.md`
> Also run
> `git diff --stat -- src-tauri/src/notifier.rs docs/TESTING_STRATEGY.md` and
> `git diff --cached --stat -- src-tauri/src/notifier.rs docs/TESTING_STRATEGY.md`.
> The latter two commands must produce no output: never overwrite concurrent
> notifier/count work. Committed drift in `notifier.rs` is a STOP; committed
> count drift is acceptable only when the live `cargo test --lib` total matches
> the section 0 row and notifier remains 22.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: security
- **Planned at**: commit `b1981c9`, 2026-07-17 (reviewed)

## Why this matters

Telegram's send URL embeds the bot token in its path. On DNS, connect, or
timeout failure, reqwest attaches that URL to `reqwest::Error`, whose `Display`
implementation appends the complete URL. The current debug log therefore
persists the token in `~/Library/Logs/notchtap/notchtap.log` during routine
debug builds. The fix must redact the URL at the production log call, and the
regression test must exercise that real call; testing reqwest's redaction API
in isolation would not protect the application.

## Current state

`src-tauri/src/notifier.rs:261-280` constructs the token-bearing URL and logs
the unredacted transport error:

```rust
// src-tauri/src/notifier.rs:267,276-280
let url = format!("{}/bot{}/sendMessage", cfg.api_base, cfg.secrets.bot_token);

let response = client.post(&url).json(&body).send().await.map_err(|e| {
    // timeouts, connect failures, dns - all transient per v3 spec section 6
    tracing::debug!("telegram request error: {e}");
    FailureKind::Transient
})?;
```

`src-tauri/src/logging.rs:35-41` enables debug logs in debug builds:

```rust
fn log_filter() -> EnvFilter {
    if cfg!(debug_assertions) {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    }
}
```

The resolved dependency is reqwest `0.13.4`
(`src-tauri/Cargo.lock:3013-3016`). Its official API is:

```rust
pub fn without_url(self) -> Self
```

It consumes the error and removes the stored URL before formatting. Reference:
<https://docs.rs/reqwest/0.13.4/reqwest/struct.Error.html#method.without_url>.

Existing patterns to match:

- Keep the test in `notifier.rs`'s existing `#[cfg(test)]` module. The
  `send_path` submodule at lines 491-593 already uses wiremock and a local
  reqwest client; add no dependency.
- Use a recognizable synthetic token only. Never read
  `~/.config/notchtap/secrets.toml` in a test; existing secret-loader tests use
  temporary fake values for the same reason.
- The sentinel style at lines 467-477 asserts that formatted errors do not
  contain synthetic secret material. Unlike that pure Display test, this new
  test must capture the tracing event emitted by `send_once` itself.
- Counts live only in `docs/TESTING_STRATEGY.md` section 0. The clean
  `b1981c9` baseline is 224 total and notifier 22. This plan still records the
  execution-time baseline rather than assuming no later plan adds tests: add
  exactly one to the total and change only notifier 22 to notifier 23.

Verified baseline caveat at `b1981c9`:

- `cargo test` passes: 224 unit/integration tests plus 3 doc-tests. Execute only
  after the in-scope working-tree drift checks are clean.
- `rustfmt --edition 2021 --check src/notifier.rs` passes.
- Repository-wide `cargo fmt --check` already fails in unrelated `settings.rs`
  formatting.
- Repository-wide `cargo clippy --all-targets -- -D warnings` already fails on
  three unrelated `too_many_arguments` lints and one test-only
  `field_reassign_with_default` lint.

Do not fix or reformat those unrelated files in this plan. The scoped clippy
command below suppresses only those two documented baseline lint classes while
still checking all test targets; the full gates remain a separately owned
baseline cleanup.

## Commands you will need

Run Rust commands from `src-tauri/`.

| Purpose | Command | Expected on success |
|---|---|---|
| Baseline module | `cargo test notifier::` | 22 pass before edits |
| Red/green test | `cargo test notifier::tests::send_path::transport_error_log_omits_token -- --exact` | fails for the specified leak before Step 2; passes after Step 2 |
| Full suite | `cargo test` | baseline total + 1 pass, plus 3 doc-tests |
| Target format | `rustfmt --edition 2021 --check src/notifier.rs` | exit 0 |
| Scoped clippy | `cargo clippy --tests -- -D warnings -A clippy::too_many_arguments -A clippy::field_reassign_with_default` | exit 0 |

## Scope

**In scope** (the only files to modify):

- `src-tauri/src/notifier.rs` - one production logging change plus one
  production-path regression test and test-only capture helper.
- `docs/TESTING_STRATEGY.md` section 0 - total and notifier test counts.
- `plans/README.md` - status row only.

**Out of scope**:

- `Cargo.toml` and `Cargo.lock`; no new test/logging crate is needed.
- Retry/backoff behavior and every other log line in `notifier.rs`.
- The drop-path warning around lines 252-253, which logs the event title but
  not the token.
- `logging.rs` filters, appenders, file permissions, or rotation.
- Existing formatting/clippy failures in `lib.rs`, `queue.rs`, `settings.rs`,
  `poller.rs`, and `rss_poller.rs`.
- Telegram credential rotation and old-log cleanup. Track those as explicit
  operator follow-ups; do not read a real token or log file while executing
  this code plan.

## Git workflow

- Prefer a clean isolated worktree. In a shared dirty worktree, record the
  initial status and do not touch pre-existing changes.
- Do not commit unless the dispatcher/operator explicitly requests it. If
  requested, match the repository's observed subject style, for example:
  `notifier: redact url from transport-error log`.
- Do not push or open a PR.

## Steps

### Step 1: Add a red production-path regression test

Before editing, run `cargo test --lib` and record its passing test count as
`BASELINE_TOTAL`. Confirm `docs/TESTING_STRATEGY.md` section 0 reports exactly
that total and `notifier 22`. If either number disagrees, STOP for count
reconciliation before writing code.

In `src-tauri/src/notifier.rs`, add the test
`send_path::transport_error_log_omits_token` before changing production code.
It must call the private `send_once` function and capture its actual tracing
output. Do not write a test that directly formats `err.without_url()`; such a
test would stay green if the production logging call regressed.

Use this exact test design:

1. Inside the existing test module, add a minimal test-only writer backed by
   `Arc<Mutex<Vec<u8>>>`. Implement `std::io::Write` for a small cloneable
   writer wrapper. Keep this helper under `#[cfg(test)]` in `notifier.rs`.
2. In a `#[tokio::test(flavor = "current_thread")]`, create a wiremock server.
   Mount a POST response on the token-bearing path with
   `ResponseTemplate::new(200).set_delay(Duration::from_millis(250))`.
3. Build a local reqwest client with a materially shorter timeout, for example
   20 ms. This deterministically produces a local timeout without DNS,
   external network, or a hard-coded supposedly-unused port.
4. Construct `WorkerConfig` with the wiremock base URL and a unique synthetic
   token marker. Do not use any real token or secrets file.
5. Build a `tracing_subscriber::fmt()` subscriber with DEBUG enabled,
   `.with_ansi(false)`, `.without_time()`, and a writer factory that returns the
   shared writer. Install it with
   `let _subscriber_guard = tracing::subscriber::set_default(subscriber);` and
   keep that guard in scope until after `send_once(...).await` and capture
   inspection. Dropping the guard earlier unregisters the subscriber. The
   current-thread Tokio flavor is required so the thread-local default remains
   active across the await.
6. Call `send_once(&client, &cfg, &event(...), false).await`.
7. Assert the result is `Err(FailureKind::Transient)`.
8. Decode the captured bytes lossily and assert the log contains the stable
   `telegram request error` prefix, proving the production branch ran.
9. Assert the log does not contain the synthetic token marker. The assertion
   message must not dump the captured buffer.

Run only the new test before touching production code:

```sh
cargo test notifier::tests::send_path::transport_error_log_omits_token -- --exact
```

**Verify (red gate)**: the test compiles, captures the expected log event, and
fails specifically because the current event contains the synthetic token. If
it passes, captures no prefix, or fails for timeout/setup reasons, STOP and
report rather than weakening the assertion.

### Step 2: Strip the URL at the production log boundary

Change only the transport-error closure in `send_once`:

```rust
let response = client.post(&url).json(&body).send().await.map_err(|e| {
    // The URL path embeds the bot token, so strip it before logging.
    tracing::debug!("telegram request error: {}", e.without_url());
    FailureKind::Transient
})?;
```

`without_url()` consumes the owned `reqwest::Error` inside `map_err`; no clone
or helper is needed. Retry classification remains `FailureKind::Transient`.

**Verify (green gate)**:
`cargo test notifier::tests::send_path::transport_error_log_omits_token -- --exact`
-> 1 passed.

### Step 3: Run the notifier suite and update counts

Run `cargo test notifier::`; it must report 23 passing notifier tests. Then
edit only the section 0 Rust row in `docs/TESTING_STRATEGY.md`:

- total: `BASELINE_TOTAL` -> `BASELINE_TOTAL + 1`
- notifier: `notifier 22` -> `notifier 23`
- leave every other module and doc-test count unchanged

**Verify**: `rg 'notifier 23' ../docs/TESTING_STRATEGY.md` returns exactly the
section 0 Rust-count row, and its leading total is exactly one greater than the
recorded `BASELINE_TOTAL`.

### Step 4: Run scoped and full verification

Run the full suite and the scoped format/lint gates from `src-tauri/`:

```sh
cargo test
rustfmt --edition 2021 --check src/notifier.rs
cargo clippy --tests -- -D warnings -A clippy::too_many_arguments -A clippy::field_reassign_with_default
```

**Verify**: `BASELINE_TOTAL + 1` tests plus 3 doc-tests pass; rustfmt exits 0;
scoped clippy exits 0. If a reviewer separately fixes the known repository
baseline before execution, also require the normal full `cargo fmt --check` and
`cargo clippy --all-targets -- -D warnings` gates to exit 0.

### Step 5: Update the plan index

Change only plan 006's status cell in `plans/README.md` from `TODO` to `DONE`.
Do not rename or move this plan file.

**Verify**:
`rg '^\| 006 .*\| DONE \|$' ../plans/README.md` -> exactly one matching row.

## Test plan

- New: one current-thread async test in `notifier.rs::tests::send_path`.
- Regression path: real `send_once` call -> deterministic local timeout -> real
  tracing event -> synthetic token absent.
- Branch proof: captured log prefix must be present, and returned failure must
  remain `FailureKind::Transient`.
- Red proof: the test must fail against the current unredacted production call
  before the fix.
- Existing: all 22 notifier tests stay green; the full Rust total increases by
  exactly one and all 3 doc-tests remain green. No live Telegram or other
  external network call is permitted.

## Done criteria

- [ ] The new production-path test was observed failing for the token leak
      before the production edit, then passing afterward.
- [ ] `cargo test notifier::` reports 23 passed and 0 failed.
- [ ] `cargo test` reports exactly `BASELINE_TOTAL + 1` passed and 3 doc-tests.
- [ ] `rustfmt --edition 2021 --check src/notifier.rs` exits 0.
- [ ] The scoped clippy command exits 0.
- [ ] `docs/TESTING_STRATEGY.md` section 0 increments only the total and
      notifier counts by one.
- [ ] No file outside the in-scope list was modified by this work.
- [ ] `plans/README.md` marks plan 006 DONE without renaming the plan.

## STOP conditions

- Any Current-state excerpt no longer matches live code semantically.
- Either working-tree/index drift command reports a pre-existing change in
  `notifier.rs` or `docs/TESTING_STRATEGY.md`.
- reqwest is no longer resolved to `0.13.4`, or `without_url()` does not
  compile with the documented consuming signature.
- The red test passes before the fix, does not capture the stable log prefix,
  or fails for setup/flakiness rather than exposure of the synthetic marker.
- The test requires external networking, a real credential, a new dependency,
  or a source-code grep in place of behavioral capture.
- A verification step fails twice after one reasonable correction.
- The change appears to require an out-of-scope file.

## Maintenance notes

- Any future log of `reqwest::Error` in this connector must account for the
  credential-bearing URL path. The production-path capture test is the
  regression guard; do not replace it with a direct unit test of reqwest.
- Operator follow-up: treat the existing Telegram bot token as potentially
  exposed, rotate it through BotFather, update
  `~/.config/notchtap/secrets.toml`, and decide whether old plaintext logs
  should be removed. Record completion without recording any token value.
- Deferred and unchanged: event titles in the drop warning and log-file mode
  are separate data-minimization concerns, not part of this fix.
