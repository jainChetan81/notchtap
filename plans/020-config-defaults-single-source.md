# Plan 020: Collapse the config-defaults triplication — serve defaults from rust via a fifth invoke command

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> This plan touches the per-window command ACL — the security-sensitive
> checklist in Step 2 is mandatory. If anything in "STOP conditions"
> occurs, stop and report. When done, update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d40445e..HEAD -- src-tauri/src/settings.rs src-tauri/build.rs src-tauri/capabilities/settings.json src/settings/SettingsApp.tsx`
> On any change, compare excerpts below; mismatch = STOP.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: MED (ACL surface change — bounded by the repo's existing gating pattern)
- **Depends on**: none. NOTE: the earlier session's plan 002 (animation
  previews) also edits SettingsApp.tsx — reconcile textually if it landed.
- **Category**: tech-debt
- **Planned at**: commit `d40445e`, 2026-07-17

## Why this matters

Config defaults are stated three times: `config.rs`'s `default_*`
functions (authoritative), `settings.rs::validate`'s ranges, and the
settings frontend's hand-maintained mirror — which self-documents the
problem:

```ts
// src/settings/SettingsApp.tsx:61-63
// Keep this synchronized with src-tauri/src/config.rs::Config::default.
// This frontend mirror can drift when Rust defaults change, so update both together.
export const DEFAULTS: Config = { … 30 lines … };
```

Every new config field pays a three-file lockstep tax (the v6 fields each
paid it), and drift fails soft (the "Reset to defaults" button silently
applies stale values). Since `Config: Serialize + Default` already holds
(the `get_config` command serializes it), a `get_default_config` command
deletes the worst copy for ~15 lines of rust. The `min=`/`max=` props
duplicating `validate` ranges are left in place — they're advisory UX,
enforcement stays server-side; a bounds-map export is not worth the
plumbing (recorded as rejected in the plans index).

## Why this is security-relevant (read before coding)

Tauri v2 allows app-defined commands to EVERY window by default. This
repo opts into deny-by-default via `tauri_build::AppManifest::commands`
in `build.rs`, grants them only to the `settings` window through
`capabilities/settings.json`, and adds a window-label check inside each
handler as defense-in-depth. **A new command MUST take all three steps
plus the `generate_handler` registration** — and
`capabilities/default.json` (the overlay's) must remain byte-for-byte
unchanged. `AGENTS.md`'s ipc section states this rule; the contract is
`docs/V5_TECHNICAL_SPEC.md` §2.

## Current state

`src-tauri/build.rs` (complete file):

```rust
fn main() {
    // v5 (V5_TECHNICAL_SPEC.md §2): tauri allows app-defined commands to
    // EVERY window by default — this opt-in flips them to deny-by-default …
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_config",
            "get_secret_status",
            "save_config_and_relaunch",
            "set_secret",
        ]),
    ))
    .expect("failed to run tauri-build");
}
```

`src-tauri/src/settings.rs:406-413` — the pattern to copy:

```rust
#[tauri::command]
pub fn get_config(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Config>,
) -> Result<Config, String> {
    ensure_settings_window(&window)?;
    Ok(state.inner().clone())
}
```

`ensure_settings_window` (same file, ~line 395) is the label gate; it has
its own test (`ensure_settings_window_gates_on_the_window_label`).
`capabilities/settings.json` grants the four existing
`allow-<command>` permissions to `windows: ["settings"]` — read it to
copy the exact permission-string format. The `generate_handler![...]`
list is in `src-tauri/src/lib.rs` (find with `rg -n "generate_handler"`).

Frontend: `SettingsApp.tsx` uses `DEFAULTS` in `resetDefaults()`
(`applyForm(DEFAULTS)`, line ~831) — find all uses with
`rg -n "DEFAULTS" src/settings/`. The invoke pattern to copy is wherever
`invoke("get_config")` is called (top of the component's load effect).
Frontend tests (`src/settings/SettingsApp.test.tsx`, 9 tests) mock IPC
via `mockIPC` — read how `get_config` is mocked; `get_default_config`
needs the same mock arm.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust suite | `cargo test` (from `src-tauri/`) | all pass |
| Frontend | `npx vitest run && npx tsc --noEmit && npx vite build` | all pass |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/settings.rs` (new command + test)
- `src-tauri/build.rs` (add to the commands array)
- `src-tauri/capabilities/settings.json` (grant the new permission)
- `src-tauri/src/lib.rs` (generate_handler list only)
- `src/settings/SettingsApp.tsx` (delete DEFAULTS; fetch defaults),
  `src/settings/SettingsApp.test.tsx` (mock arm + reset test)
- `docs/V5_TECHNICAL_SPEC.md` §2 (command table gains a row — working
  draft, designed to be edited), `docs/TESTING_STRATEGY.md` §0
- `plans/README.md` (status row)

**Out of scope**:
- `src-tauri/capabilities/default.json` — MUST NOT CHANGE (verify in
  done criteria).
- The `min=`/`max=` advisory props and `validate`'s ranges (recorded
  as accepted duplication).
- The other four commands' behavior.

## Git workflow

- Current branch; commit style: `settings: get_default_config invoke — delete the frontend DEFAULTS mirror`.
- Do NOT push.

## Steps

### Step 1: The command (rust)

In `settings.rs`, next to `get_config`:

```rust
/// Serves Config::default() so the frontend never mirrors defaults
/// (plan 020) — the "Reset to defaults" source of truth is config.rs.
#[tauri::command]
pub fn get_default_config(window: tauri::WebviewWindow) -> Result<Config, String> {
    ensure_settings_window(&window)?;
    Ok(Config::default())
}
```

Add `"get_default_config"` to build.rs's commands array; add the
corresponding `allow-get-default-config`-style permission to
`capabilities/settings.json` (copy the exact naming convention of the
existing four — read the file; tauri autogenerates the permission from
the command name); add the fn to `generate_handler![...]` in lib.rs.

Add a rust test mirroring the existing command tests: the label gate
rejects a non-settings window, and the returned value equals
`Config::default()` (serialize both to `serde_json::Value` and compare —
avoids requiring `PartialEq`).

**Verify**: `cargo test` → all pass incl. the new test; `cargo clippy --all-targets -- -D warnings` → exit 0. `git diff --stat src-tauri/capabilities/default.json` → empty.

### Step 2: ACL checklist (mandatory)

Confirm all four legs exist for the new command (list them in your
report): (1) `#[tauri::command]` + `ensure_settings_window` first line;
(2) build.rs array entry; (3) settings.json permission granted to
`windows: ["settings"]` only; (4) `generate_handler` registration.
Also confirm `capabilities/default.json` is untouched.

**Verify**: `grep -c "get_default_config" src-tauri/build.rs src-tauri/capabilities/settings.json src-tauri/src/lib.rs src-tauri/src/settings.rs` → ≥1 in each file.

### Step 3: Frontend — delete the mirror

In `SettingsApp.tsx`:
- Remove `export const DEFAULTS: Config = {…}` and its "keep
  synchronized" comment.
- In the load effect where `get_config` is invoked, also invoke
  `get_default_config` (parallel `Promise.all` is fine) and keep the
  result in state (e.g. `const [defaults, setDefaults] = useState<Config | null>(null)`).
- `resetDefaults()` → `if (defaults) applyForm(defaults);` (and disable
  the reset button until defaults have loaded, matching however the form
  already handles the pre-`get_config` loading state — read the
  component's existing loading handling and copy it).
- Fix any other `DEFAULTS` references the grep finds.

In `SettingsApp.test.tsx`: add the `get_default_config` arm to the IPC
mock (returning the same fixture object the tests use for `get_config`,
or a distinct one if a test asserts reset semantics); if no test covers
"Reset to defaults", add one: load the form with non-default values,
click reset, assert a known field shows the mocked default.

**Verify**: `npx vitest run` → all pass incl. the reset test; `npx tsc --noEmit` → exit 0 (proves no dangling DEFAULTS references); `npx vite build` → exit 0.

### Step 4: Docs

`docs/V5_TECHNICAL_SPEC.md` §2: add `get_default_config` to the command
table (read-only, no side effects, settings-window-gated).
`docs/TESTING_STRATEGY.md` §0: settings +1 (rust), frontend +1.

**Verify**: full gates green.

### Step 5: Manual ACL smoke (operator, once)

Same discipline as the v5 exit criteria: from the *main* window's
devtools console, `window.__TAURI__.core.invoke("get_default_config")`
must be DENIED; from the settings window it succeeds. (Requires a dev
build; hand to operator if not runnable.)

**Verify**: operator confirmation or explicitly reported pending.

## Test plan

- Rust: 1–2 tests (label gate + default-equality) modeled on the
  existing settings command tests.
- Frontend: 1 mock arm + 1 reset-to-defaults interaction test modeled on
  the existing SettingsApp tests.

## Done criteria

- [ ] `grep -c "const DEFAULTS" src/settings/SettingsApp.tsx` → 0
- [ ] Step 2's four-leg checklist reported complete
- [ ] `git diff d40445e..HEAD -- src-tauri/capabilities/default.json` → empty
- [ ] `cargo test`, clippy, fmt, `npx vitest run`, `npx tsc --noEmit`, `npx vite build` all exit 0
- [ ] V5 spec §2 + TESTING_STRATEGY §0 updated
- [ ] Manual ACL smoke reported
- [ ] `plans/README.md` status row updated

## STOP conditions

- The autogenerated permission name for the new command can't be
  determined from settings.json's existing entries — check
  `src-tauri/gen/` or tauri docs; if still ambiguous, report (do NOT
  loosen the capability to a wildcard).
- Anything requires editing `capabilities/default.json`.
- The earlier session's plan 002 restructured SettingsApp.tsx's
  load/reset flow — reconcile by reading; STOP if the DEFAULTS usage
  moved somewhere this plan doesn't describe.

## Maintenance notes

- New config fields now touch two places (config.rs default + validate
  range) instead of three; the frontend picks up defaults automatically.
  Reviewers of future config PRs: check there's no re-introduced TS
  mirror.
- The advisory `min=`/`max=` props still duplicate validate ranges — 
  accepted; if a field's range ever changes, grep SettingsApp.tsx for the
  old bound.
- This sets the precedent for command #6+: copy Step 2's four-leg
  checklist into any future command's PR description.
