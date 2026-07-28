# Plan 155: Stop the Kimi version probe from blocking the agent hot path, and range-check the `[agents]` durations

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat acdaeb0..HEAD -- src-tauri/src/agents/health.rs src-tauri/src/agents/providers/kimi_version.rs src-tauri/src/settings.rs src-tauri/src/config.rs src-tauri/src/agents/registry.rs src-tauri/src/agents/board.rs src-tauri/src/http.rs src-tauri/src/bin/notchtap_agent.rs src/settings/sections/AgentsSection.tsx docs/TESTING_STRATEGY.md`
> If any changed since this plan was written, compare the "Current state"
> excerpts against the live code before proceeding; on a mismatch, treat it
> as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW-MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `acdaeb0`, 2026-07-28

## Why this matters

Two independent defects on the same surface — the Agent Adapter
configuration and health path. They are batched because they touch
adjacent code and share a verification run, not because they are the
same bug.

**Part A.** Every 60 seconds, notchtap shells out to `kimi --version`
**while holding a `std::sync::Mutex`**, from a `tokio` worker thread,
with **no timeout**. The lock it holds is the same one the
`/agent/events` request handler takes to record accepted events. So if
`kimi` is a slow wrapper script, waits on stdin, or hangs on a network
call, it does not merely delay a health read — it blocks agent event
ingestion and every Agent Board publish, indefinitely. Worse, the probe
runs **unconditionally**, before the per-runtime enabled check, so a user
who disabled Kimi in Settings, or never installed it, pays the same cost.
The same unbounded probe also runs inside `notchtap-agent hook kimi`,
whose entire contract is that it must never block the provider process
that spawned it.

**Honest scope of the fix**: after this plan, the worst case is a
**bounded ~750 ms stall once per 60-second cache window**, still on a
tokio worker. That is a large improvement over unbounded, and it removes
the lock from the picture entirely — but it does not make the probe
free. Making `snapshot` async would, and is deliberately out of scope
(see Scope, and Maintenance notes).

**Part B.** `agents.stale_after_secs`, `agents.terminal_retention_secs`
and `agents.stale_retention_secs` are the only numeric config family in
the app with **no** server-side range check, and the Settings window
offers `min={0}` for all three. Typing `0` into "Stale threshold" saves
successfully; on relaunch the first 5-second board tick marks *every*
session Stale — including one that is actively Working. The Agent Board
is then permanently empty, with no error and nothing in the UI to explain
it.

## Current state

### Part A — the probe

`src-tauri/src/agents/health.rs:411-421`, verbatim:

```rust
    pub fn kimi_hook_support(&self, now: Instant) -> HookSupport {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((probed_at, support)) = &guard.kimi_cache {
            if now.saturating_duration_since(*probed_at) < KIMI_PROBE_CACHE_TTL {
                return support.clone();
            }
        }
        let support = kimi_version::probe_hook_support();
        guard.kimi_cache = Some((now, support.clone()));
        support
    }
```

`guard` is alive across `probe_hook_support()`. That is the bug: the lock
is held for the whole duration of a process spawn.

`src-tauri/src/agents/health.rs:426-428` — the probe is unconditional:

```rust
    pub fn snapshot(&self, runtimes_cfg: &AgentRuntimesConfig, now: Instant) -> Vec<AdapterHealth> {
        let kimi_hook = self.kimi_hook_support(now);
        ALL_RUNTIMES
```

The per-runtime enabled flag is only consulted later, at
`health.rs:441`, inside the map, which reads (`health.rs:439-445`):

```rust
                build_adapter_health(
                    runtime,
                    runtimes_cfg.runtime_enabled(runtime),
                    if runtime == AgentRuntime::Kimi {
                        Some(&kimi_hook)
                    } else {
                        None
                    },
```

`HealthTracker` holds a single `StdMutex<TrackerInner>`
(`health.rs:341-343`); `TrackerInner` carries both the per-runtime
`records` map and `kimi_cache: Option<(Instant, HookSupport)>`
(`health.rs:333`). `record_accepted` / `record_error`, called from
`http.rs`'s `/agent/events` handler, take that same lock — which is why
holding it across a subprocess is not merely a health-read problem.

`KIMI_PROBE_CACHE_TTL` is 60 seconds (`health.rs:41`).

The probe itself. **`src-tauri/src/agents/providers/kimi_version.rs:117-125`**,
verbatim:

```rust
pub fn detect_installed_version() -> Option<String> {
    let output = std::process::Command::new("kimi")
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
```

`Command::output()` blocks until the child exits. There is no timeout
anywhere in this path.

**Important**: `kimi_version.rs` has **no module-level `use` statements
at all** — the current code writes `std::process::Command::new(...)`
fully qualified. The only `use` in the file is `use super::*;` inside
`mod tests` (at `kimi_version.rs:149`). Step 1 tells you exactly which
imports to add.

Callers of `snapshot` (both reach the probe today):
- `src-tauri/src/agents/board.rs:364-366`, inside the async
  `publish_if_changed`, which the 5-second tick loop
  (`board.rs:427-430`) and the `/agent/events` handler
  (`src-tauri/src/http.rs:480`) both drive.
- the Settings `get_agent_health` command (`settings.rs:1138`).

Callers of `probe_hook_support` directly:
- `src-tauri/src/bin/notchtap_agent.rs:159` (`deliver_kimi`) — runs on
  every Kimi hook event. `notchtap_agent.rs:24-25` documents that hook
  mode "ALWAYS exits 0 (fail open — a provider session must never be
  blocked by notchtap's absence or a delivery failure)".
- `src-tauri/src/bin/notchtap_agent.rs:268` (`run_status`) — interactive.

**The load-bearing safety fact for Step 2.** `snapshot` does not call
`availability_for`/`compatibility_message` directly — it calls
`build_adapter_health` (`health.rs:285-301`), which is the **sole**
consumer of the `Option<&HookSupport>` argument and threads it into
exactly those two functions and nowhere else. Both return before reading
`kimi_hook` when the runtime is disabled:

```rust
// health.rs:219-226
pub fn availability_for(
    runtime: AgentRuntime,
    enabled: bool,
    kimi_hook: Option<&HookSupport>,
) -> AdapterAvailability {
    if !enabled {
        return AdapterAvailability::Unavailable;
    }
```

`compatibility_message` (`health.rs:243-249`) has the same early return.
So passing `None` for a disabled Kimi is behaviour-preserving. **Verify
all three functions yourself in Step 2** — `build_adapter_health` included,
not just the two pure ones.

### Part B — the missing range checks

`src-tauri/src/settings.rs`'s `validate` (starts at `:90`) checks
`port`, `default_ttl`, `max_queued_per_tier`, `espn_poll_secs`,
`espn_ttl_secs`, `agent_ttl_secs`, `weather_ttl_secs`, the ESPN league
slugs, the rotation-order permutation, and the RSS fields. It contains
**no** check for any `agents.*` field.

`src-tauri/src/config.rs:226-245` is the struct. It is **not** a flat
list — the fields carry multi-line `///` doc comments, and there are ten
fields, not three. The three this plan touches, verbatim (doc comments
elided with `[...]` markers so you can see the real shape):

```rust
pub struct AgentsConfig {
    pub enabled: bool,
    /// [3-line doc comment about the 600 -> 60 operator decision]
    pub terminal_retention_secs: u64,
    pub stale_after_secs: u64,
    /// [4-line doc comment about mirroring terminal_retention_secs]
    pub stale_retention_secs: u64,
    pub informational_notifications: bool,
    pub permission_priority: Priority,
    pub input_priority: Priority,
    pub failure_priority: Priority,
    pub completion_priority: Priority,
    pub runtimes: AgentRuntimesConfig,
}
```

Defaults are at `config.rs:249-266`: `terminal_retention_secs: 60`,
`stale_after_secs: 300`, `stale_retention_secs: 600`.

**Critically, `AgentsConfig` is a nested struct**: it hangs off
`Config.agents` (`config.rs:96`), not off `Config` directly. Step 4's
tests must account for that — see the Test plan, which gives the exact
literal form.

The consequence, `src-tauri/src/agents/registry.rs:317`:

```rust
            if now.saturating_duration_since(session.last_seen_at) >= self.stale_after {
```

With `stale_after == Duration::ZERO` this is unconditionally true on
every tick, for every session.

The Settings controls, `src/settings/sections/AgentsSection.tsx:428-460`
— three `NumberControl`s, all `min={0} max={86400}`, at `:433`, `:445`,
`:455`. The one to change, verbatim (`:441-449`):

```tsx
        <NumberControl
          id="agents-stale-after"
          name="Stale threshold"
          help="A session with no accepted event for this long is marked Stale on the Agent Board."
          value={config.agents.stale_after_secs}
          min={0}
          max={86400}
          unit="SEC"
          onChange={(stale_after_secs) => patchAgents(config, patchConfig, { stale_after_secs })}
        />
```

The exemplar for the new validate rules is `weather_ttl_secs` — the rule
at `settings.rs:128-133` and the boundary test `weather_ttl_boundaries`
at `settings.rs:1341-1354`, verbatim:

```rust
    #[test]
    fn weather_ttl_boundaries() {
        let mut c = Config {
            weather_ttl_secs: 0,
            ..Config::default()
        };
        assert!(validate(&c).is_err());
        c.weather_ttl_secs = 1;
        assert!(validate(&c).is_ok());
        c.weather_ttl_secs = 3600;
        assert!(validate(&c).is_ok());
        c.weather_ttl_secs = 3601;
        assert!(validate(&c).is_err());
    }
```

Copy its *structure* only. `weather_ttl_secs` is a direct `Config` field;
yours are not.

`validate` returns `Result<(), Vec<String>>` and accumulates **all**
violations rather than short-circuiting — its doc says "Every rule
violated contributes one human-readable message — the settings form
renders the whole list, not just the first failure."

### Repo conventions you must follow

1. **Pure / impure split.** `CLAUDE.md:158-161`: *"keep the pure decision
   logic … separate from that subprocess call — the function is
   unit-testable, the subprocess call is not."* `kimi_version.rs:6-13`
   restates it for this exact file: `hook_support` is pure,
   `detect_installed_version` is the isolated impure probe. Your new
   bounded runner takes the program and args as parameters so a test can
   point it at a different binary.
2. **Error handling** (`CLAUDE.md:213-219`): `thiserror` for library
   modules, `anyhow` at boundaries. Neither is needed here — both
   functions already return `Option` / `Result<(), Vec<String>>`. Do not
   introduce an error type.
3. **No new dependencies.** Do not add `wait-timeout` or similar.
   `std::process` plus `std::time` is sufficient.
4. **Tests live in an in-file `#[cfg(test)] mod tests`.**
5. **Test counts live in `docs/TESTING_STRATEGY.md` §0 and nowhere else**
   (`CLAUDE.md:19`).

### Vocabulary (`CONTEXT.md` — use these words in comments and messages)

**Agent Runtime**, **Agent Adapter**, **Agent Registry**, **Agent
Board**, **Agent Session**, **Stale**, **Terminal Retention**.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | exit 0, `0 failed` |
| Probe tests | `cd src-tauri && cargo test --locked kimi_version` | exit 0 |
| Health tests | `cd src-tauri && cargo test --locked agents::health` | exit 0 |
| Settings tests | `cd src-tauri && cargo test --locked settings::tests` | exit 0 |
| Rust format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Rust lint | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Frontend tests | `npx vitest run` | exit 0, all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Frontend lint | `npx biome ci .` | exit 0 |

If `cargo` is not on PATH, prefix with `PATH="$HOME/.cargo/bin:$PATH"`.
A cold `src-tauri/target/` makes the first `cargo` command take several
minutes. That is normal.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/agents/providers/kimi_version.rs`
- `src-tauri/src/agents/health.rs`
- `src-tauri/src/settings.rs`
- `src/settings/sections/AgentsSection.tsx`
- `docs/TESTING_STRATEGY.md` §0 (recount, last step)
- `plans/README.md` (status row)

**Out of scope** (do NOT touch):

- `src-tauri/src/agents/registry.rs` — the `stale_after` comparison at
  `:317` is correct given a sane config; this plan fixes the config, not
  the comparison. Do not add a defensive clamp there as well; two clamps
  in two layers is how a config value silently stops meaning what it says.
- `src-tauri/src/agents/board.rs`, `src-tauri/src/http.rs`,
  `src-tauri/src/bin/notchtap_agent.rs` — the callers. `snapshot`'s
  signature does not change.
- `src/settings/SettingsApp.test.tsx` — **checked: it contains no
  assertion on any `min` prop or on the stale-threshold help text** (its
  only agents-duration assertions are `getByDisplayValue` calls on the
  values). Do not add one; do not go looking.
- The `[silence]` config block — its own parse-time validation, separate
  scope.
- `MINIMUM_HOOK_VERSION` / `MINIMUM_HOOK_VERSION_STR` —
  `kimi_version.rs:15-36` flags the value as needing manual verification.
  Leave exactly as is.
- Making `snapshot` or `kimi_hook_support` async.
- `src-tauri/capabilities/*` — must not change. No new
  `#[tauri::command]`; the count stays seventeen.

## Git workflow

- Branch: `advisor/155-kimi-probe-and-agents-config-bounds`
- Conventional-commit style, e.g.
  `fix(agents): bound the kimi version probe and range-check [agents] durations`
- Commit Part A (Steps 1–3) and Part B (Steps 4–5) separately.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Add the bounded process runner in `kimi_version.rs`

**1a. Add the imports.** The file currently has none at module level.
Add this block immediately after the module doc comment, before
`pub const MINIMUM_HOOK_VERSION`:

```rust
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
```

(`std::thread::sleep` is called fully qualified below, matching this
file's existing fully-qualified style for one-off calls.)

**1b. Add the constants and the runner**, above `detect_installed_version`.
This is the complete target code — write it as given:

```rust
/// Hard ceiling on the `kimi --version` probe. Matches the 750ms budget
/// `providers::delivery` already uses for its POST (`delivery.rs:33`) —
/// the same "a helper must never make the caller wait perceptibly" rule,
/// applied to the one other place this crate blocks on something it does
/// not control.
const PROBE_TIMEOUT: Duration = Duration::from_millis(750);

/// How often the bounded wait polls the child. 10ms keeps the normal
/// case (a `--version` that returns in a few ms) from paying a
/// meaningful sampling penalty, at 75 polls worst case.
const PROBE_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Runs `program args…` with a hard time budget, returning its stdout on
/// a successful, in-budget exit and `None` otherwise (spawn failure,
/// non-zero exit, or timeout — in which case the child is killed and
/// reaped before returning).
///
/// This exists because `Command::output()` blocks until the child exits,
/// with no ceiling. That is a latent hang in two places: the
/// `HealthTracker` probe runs on a tokio worker on the same path as
/// `/agent/events` ingestion, and `notchtap-agent hook kimi` must never
/// block the provider process that spawned it (see that binary's module
/// doc: hook mode "ALWAYS exits 0 … a provider session must never be
/// blocked").
///
/// LIMITATION, deliberate: stdout is piped and read only AFTER the child
/// exits, so any command whose output exceeds the OS pipe buffer (64 KiB
/// on macOS) will block on `write`, never exit, and therefore ALWAYS hit
/// the timeout and return `None` — no matter how large the budget. It
/// cannot deadlock (the budget always fires), but it also cannot ever
/// succeed. Only pass commands with short, bounded output.
///
/// `program`/`args` are parameters rather than hardcoded so the bounded
/// behaviour is unit-testable against a binary guaranteed to be present,
/// without needing `kimi` installed — the same reason [`hook_support`] is
/// split from [`detect_installed_version`].
fn run_bounded(program: &str, args: &[&str], budget: Duration) -> Option<Vec<u8>> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now().checked_add(budget)?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    // Already reaped by `try_wait`; this `wait` returns
                    // the cached status without blocking, and keeps every
                    // exit path symmetric.
                    let _ = child.wait();
                    return None;
                }
                // Valid after `try_wait` returned `Some`: `wait` short-
                // circuits on the cached status, and the stdout pipe is
                // still readable because we own the read end.
                return child.wait_with_output().ok().map(|o| o.stdout);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(PROBE_POLL_INTERVAL);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}
```

Note `let _ =` on **every** `kill()` and `wait()` — both return
`#[must_use]` results and `-D warnings` rejects a bare call.

**1c. Rewrite `detect_installed_version`** to use it, keeping its
existing doc comment and its `None`-means-unavailable contract. Write the
tail in rustfmt's block form, not a one-liner:

```rust
pub fn detect_installed_version() -> Option<String> {
    let stdout = run_bounded("kimi", &["--version"], PROBE_TIMEOUT)?;
    let text = String::from_utf8(stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
```

**If `cargo clippy --locked --all-targets -- -D warnings` reports
`clippy::zombie_processes`** on `run_bounded` despite every path calling
`wait`, you are pre-authorised to add
`#[allow(clippy::zombie_processes)]` on the function with a one-line
comment stating that every return path reaps the child and the lint is
being conservative. Do not restructure the function to appease it, and do
not add a crate-wide allow.

**Verify**: all three:
- `cd src-tauri && cargo build --lib` → exit 0
- `cd src-tauri && cargo fmt --check` → exit 0
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0

### Step 2: Take the probe out of the lock, and skip it when Kimi is off

**2a. Confirm the safety claim first.** Read three functions and confirm
each holds before changing anything:
- `health.rs:219-226` — `availability_for` returns before reading
  `kimi_hook` when `!enabled`.
- `health.rs:243-249` — `compatibility_message` does the same.
- `health.rs:285-301` — `build_adapter_health` threads `kimi_hook` into
  **only** those two calls and nowhere else.

If any of the three does not hold, **STOP and report**.

**2b. Rewrite `kimi_hook_support`** (`health.rs:411-421`) to this exact
shape:

```rust
    pub fn kimi_hook_support(&self, now: Instant) -> HookSupport {
        {
            let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((probed_at, support)) = &guard.kimi_cache {
                if now.saturating_duration_since(*probed_at) < KIMI_PROBE_CACHE_TTL {
                    return support.clone();
                }
            }
        } // guard dropped HERE — the probe below spawns a process, and
          // this same mutex guards `records`, which `/agent/events`
          // writes to on every accepted event. Holding it across a
          // subprocess would stall ingestion, not just health reads.
        let support = kimi_version::probe_hook_support();
        // Deliberate, benign race: two callers arriving together on an
        // expired cache may both probe. Cost is one extra bounded
        // `kimi --version`; the alternative (holding the lock) is the bug
        // being fixed. `records` and `kimi_cache` are structurally
        // independent — no invariant couples them — so nothing can
        // observe a torn state. The only visible effect is that a
        // later-finishing caller may store an EARLIER `now`, marginally
        // shortening the effective cache TTL.
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.kimi_cache = Some((now, support.clone()));
        support
    }
```

**2c. Gate the probe in `snapshot`.** Replace `health.rs:427` with:

```rust
        // Probe ONLY when Kimi is enabled: `build_adapter_health` passes
        // `kimi_hook` to exactly `availability_for` and
        // `compatibility_message`, and both return before reading it when
        // `enabled` is false — so `None` is behaviour-preserving for a
        // disabled runtime. A user who never installed Kimi should not pay
        // a process spawn every `KIMI_PROBE_CACHE_TTL` forever.
        let kimi_hook = if runtimes_cfg.runtime_enabled(AgentRuntime::Kimi) {
            Some(self.kimi_hook_support(now))
        } else {
            None
        };
```

Then change the `build_adapter_health` argument at `health.rs:441-445`.
The current code is:

```rust
                    if runtime == AgentRuntime::Kimi {
                        Some(&kimi_hook)
                    } else {
                        None
                    },
```

Replace it with **exactly** this — keep the `if/else`; do not "simplify"
to a bare `kimi_hook.as_ref()`, which would pass the Kimi hook to all
four runtimes and violate `build_adapter_health`'s documented contract:

```rust
                    if runtime == AgentRuntime::Kimi {
                        kimi_hook.as_ref()
                    } else {
                        None
                    },
```

**Verify**: `cd src-tauri && cargo test --locked agents::health` → exit 0,
all existing health tests still pass unchanged.

### Step 3: Tests for Part A

Write the tests listed under "Test plan / Part A" into the existing
`#[cfg(test)] mod tests` in `kimi_version.rs` (at `:149`) and
`health.rs` (at `:455`).

**Verify**: `cd src-tauri && cargo test --locked kimi_version 2>&1 | grep 'test result'`
→ a `test result: ok.` line whose passed count is **4 higher** than the
count you record before writing them. Record that baseline first.

### Step 4: Add the three validate rules

In `src-tauri/src/settings.rs`'s `validate`, immediately after the
`weather_ttl_secs` rule at `:128-133`, add this comment and three checks.
Push onto `errors`; never early-return:

```rust
    // The asymmetry below is deliberate, not an oversight. Zero RETENTION
    // is a legitimate choice ("drop a finished or stale session on the
    // next board tick"). A zero stale THRESHOLD is never meaningful — it
    // marks every Agent Session Stale on the first tick, including one
    // that is actively Working, which empties the Agent Board with no
    // error. Do not "tidy" these into one shared range.
    if !(1..=86400).contains(&c.agents.stale_after_secs) {
        errors.push(format!(
            "agents.stale_after_secs must be 1–86400 seconds (got {}) — 0 marks every Agent Session Stale on the first board tick",
            c.agents.stale_after_secs
        ));
    }
    if !(0..=86400).contains(&c.agents.terminal_retention_secs) {
        errors.push(format!(
            "agents.terminal_retention_secs must be 0–86400 seconds (got {})",
            c.agents.terminal_retention_secs
        ));
    }
    if !(0..=86400).contains(&c.agents.stale_retention_secs) {
        errors.push(format!(
            "agents.stale_retention_secs must be 0–86400 seconds (got {})",
            c.agents.stale_retention_secs
        ));
    }
```

`86400` matches the ceiling the Settings UI already advertises
(`AgentsSection.tsx:434,446,456`); keep them equal.

**Verify**: `cd src-tauri && cargo test --locked settings::tests` → exit 0
(existing tests, including `default_config_validates_clean` at
`settings.rs:1266`, must still pass).

### Step 5: Raise the Settings floor, and test Part B

**5a.** In `src/settings/sections/AgentsSection.tsx`, change **only** the
`agents-stale-after` control's `min={0}` (at `:445`) to `min={1}`. Leave
`agents-terminal-retention` (`:433`) and `agents-stale-retention`
(`:455`) at `min={0}` — zero is valid for both, per Step 4.

Extend that one control's `help` string to:
`"A session with no accepted event for this long is marked Stale on the Agent Board. Must be at least 1 second."`

**5b.** Write the Part B tests (see Test plan) into `settings.rs`'s
existing `mod tests`.

**Verify**: all of
- `npx vitest run` → exit 0
- `npx tsc --noEmit` → exit 0
- `npx biome ci .` → exit 0
- `grep -c 'min={0}' src/settings/sections/AgentsSection.tsx` → `2`
- `cd src-tauri && cargo test --locked settings::tests` → exit 0

### Step 6: Recount and gate

Recount `docs/TESTING_STRATEGY.md` §0 from live runs of both suites. §0's
rust row is one long cell that accretes `+N with plan NNN (…)` clauses —
follow that existing convention: append one clause naming plan 155 and
the tests you added, and update the row's running total to match your
live count. Do not restate counts anywhere else in the repo.

**Verify**: all of
- `cd src-tauri && cargo fmt --check` → exit 0
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo test --locked` → `0 failed`
- `npx vitest run` → exit 0
- `npx tsc --noEmit` → exit 0
- `npx biome ci .` → exit 0

## Test plan

### Part A — `kimi_version.rs` (4 new tests)

Add to the existing `mod tests`. These use binaries guaranteed present on
macOS, and the rust CI job is `runs-on: macos-latest`
(`.github/workflows/ci.yml:17`), so no `kimi` install is needed.

1. `run_bounded_returns_stdout_on_success` —
   `run_bounded("/bin/echo", &["hello"], Duration::from_secs(5))` →
   `Some(bytes)` whose UTF-8 trims to `"hello"`.
2. `run_bounded_kills_a_child_that_outlives_its_budget` —
   `let start = Instant::now();`
   `run_bounded("/bin/sleep", &["5"], Duration::from_millis(100))` →
   `None`, and `assert!(start.elapsed() < Duration::from_secs(2));`.
   Add this comment: *"Deliberate real-timer test. The behaviour under
   test IS wall-clock process termination, which cannot be simulated. Cost
   is bounded at ~100ms; the 2s assertion ceiling is a 20× margin."* Do
   not claim a documented precedent or a count of other real-timer tests
   — this repo keeps no such register.
3. `run_bounded_rejects_a_non_zero_exit` —
   `run_bounded("/bin/sh", &["-c", "exit 3"], Duration::from_secs(5))` → `None`.
4. `run_bounded_returns_none_when_the_program_does_not_exist` —
   `run_bounded("/nonexistent/notchtap-probe-test", &[], Duration::from_secs(5))`
   → `None`, no panic.

The existing `hook_support` / `parse_semver_prefix` tests must still pass
untouched — the pure half does not change.

### Part A — `health.rs` (1 new test)

5. `snapshot_skips_the_kimi_probe_when_kimi_is_disabled` — build an
   `AgentRuntimesConfig` with `kimi.enabled = false`, call `snapshot`,
   assert the Kimi row's `availability` is `AdapterAvailability::Unavailable`
   and its `compatibility_message` is the disabled message. Model on the
   existing `snapshot_reflects_a_disabled_runtime_toggle`
   (`health.rs:677`), which does exactly this for Codex — copy its
   config-construction idiom. Comment that "no process was spawned"
   cannot be asserted without injection, so this pins the observable
   contract instead.

**Do not add a cache-TTL test** — `kimi_hook_probe_is_cached_within_the_ttl`
already exists at `health.rs:711-724`. Confirm it still passes.

### Part B — `settings.rs` (3 new tests)

`AgentsConfig` is nested under `Config.agents`, so the
`..Config::default()` idiom from `weather_ttl_boundaries` does not apply
directly. There is no existing `agents:` literal in `settings.rs`'s tests
to copy, so use exactly this form:

```rust
    #[test]
    fn agents_stale_after_boundaries() {
        let mut c = Config::default();
        c.agents.stale_after_secs = 0;
        assert!(validate(&c).is_err());
        c.agents.stale_after_secs = 1;
        assert!(validate(&c).is_ok());
        c.agents.stale_after_secs = 86400;
        assert!(validate(&c).is_ok());
        c.agents.stale_after_secs = 86401;
        assert!(validate(&c).is_err());
    }
```

6. `agents_stale_after_boundaries` — exactly as above.
7. `agents_retention_boundaries` — for **both** `terminal_retention_secs`
   and `stale_retention_secs`: `0` is **ok**, `86400` ok, `86401` errors.
   This test is what stops someone later "fixing" the asymmetry.
8. `agents_duration_violations_are_all_reported_together` — set
   `stale_after_secs = 0`, `terminal_retention_secs = 86401`,
   `stale_retention_secs = 86401` (note the values: all three must be
   genuinely out of *their own* range, and `0` is in range for the two
   retentions), then assert `validate(&c).unwrap_err().len() == 3`.

## Done criteria

ALL must hold. Each is a command with an expected result:

- [ ] `cd src-tauri && cargo test --locked` → `0 failed`
- [ ] `cd src-tauri && cargo test --locked kimi_version 2>&1 | grep -c 'run_bounded'` → `0` (test names don't print on success; instead confirm the pass count rose by 4 against the Step 3 baseline you recorded)
- [ ] `cd src-tauri && cargo fmt --check` → exit 0
- [ ] `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .` → all exit 0
- [ ] `grep -c 'fn run_bounded' src-tauri/src/agents/providers/kimi_version.rs` → `1`
- [ ] `grep -c '\.output()' src-tauri/src/agents/providers/kimi_version.rs` → `0`
- [ ] `grep -c 'agents.stale_after_secs must be' src-tauri/src/settings.rs` → `1`
- [ ] `grep -c 'fn agents_stale_after_boundaries' src-tauri/src/settings.rs` → `1`
- [ ] `grep -c 'fn agents_retention_boundaries' src-tauri/src/settings.rs` → `1`
- [ ] `grep -c 'min={0}' src/settings/sections/AgentsSection.tsx` → `2`
- [ ] `grep -c 'min={1}' src/settings/sections/AgentsSection.tsx` → `1`
- [ ] `awk '/pub fn kimi_hook_support/,/^    }$/' src-tauri/src/agents/health.rs | grep -n 'probe_hook_support'` returns a line that comes **after** the line matching `} // guard dropped HERE` in the same output
- [ ] `git diff --stat src-tauri/capabilities/` → empty
- [ ] `grep -r '#\[tauri::command\]' src-tauri/src | wc -l` → `17`
- [ ] `git status --porcelain` lists only files from the In-scope list
- [ ] `docs/TESTING_STRATEGY.md` §0 recounted from a live run
- [ ] `plans/README.md` status row updated (skip if your reviewer maintains the index)

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" doesn't match the
  excerpts — especially `kimi_hook_support`'s body, `snapshot`'s first
  line, or `validate`'s structure.
- Any of `availability_for`, `compatibility_message`, or
  `build_adapter_health` turns out **not** to behave as Step 2a
  describes. The gate depends on all three; without them, skipping the
  probe would change what a disabled Kimi reports, which this plan does
  not authorise.
- An existing test asserts that `snapshot` probes unconditionally, or
  otherwise pins the current lock-holding behaviour. That would mean the
  behaviour was deliberate and this plan's premise is wrong — report it,
  do not delete the test.
- `cargo clippy` rejects `run_bounded` for a reason **other than**
  `clippy::zombie_processes` (which Step 1 pre-authorises an `#[allow]`
  for). Report the exact lint rather than restructuring.
- Test 2 (`/bin/sleep` timeout) fails or is flaky across three
  consecutive runs. Report rather than widening the budget until it
  passes.
- You conclude the fix requires making `snapshot` or `kimi_hook_support`
  async, or changing any caller in `board.rs` / `http.rs` /
  `notchtap_agent.rs`.
- Any gate fails twice after a reasonable fix attempt.

## Maintenance notes

- **`run_bounded` is deliberately not general-purpose.** It reads stdout
  only after the child exits, so any command whose output exceeds the
  pipe buffer will *always* time out and return `None` — it will look
  like "the binary is slow", not like a hang. If a second caller ever
  appears, either keep it to short-output commands or add a reader
  thread.
- **`kill()` signals only the direct child.** If `kimi` is a wrapper
  script — the exact case "Why this matters" cites — a grandchild
  survives. `run_bounded` still returns on time; the orphan is not
  reaped by us. Process-group killing was considered out of proportion
  here.
- **The benign double-probe race** in Step 2b is the accepted cost of not
  holding the lock. If someone later "fixes" it by re-widening the
  critical section, they have reintroduced this bug. A reviewer should
  check for exactly that.
- **The residual stall is ~750 ms per 60 s**, still on a tokio worker.
  Removing it entirely means making `snapshot` async, which ripples into
  `board.rs` and `settings.rs`. Deliberately deferred.
- **`notchtap-agent hook kimi` benefits for free**: `deliver_kimi`
  (`notchtap_agent.rs:159`) calls `probe_hook_support` on every Kimi hook
  event, so the timeout now bounds the worst case where a hung `kimi`
  would have delayed a provider's hook. Worth mentioning in the commit.
- **The `86400` ceiling is duplicated** between `settings.rs`'s rules and
  `AgentsSection.tsx`'s `max`. This repo has a recorded decision that
  advisory `min`/`max` props duplicating `validate` ranges is accepted
  (enforcement is server-side; a shared bounds export "isn't worth the
  plumbing"). Do not extract a shared constant — keep the numbers equal.
- **Deliberately deferred**: bounding the Agent Registry's session count
  and the Agent Board's published snapshot size (audit finding F11), and
  `[silence]` block validation.
