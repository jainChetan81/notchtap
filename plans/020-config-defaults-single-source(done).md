# Plan 020: Collapse the config-defaults triplication — serve defaults from rust via a fifth invoke command

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> This plan touches the per-window command ACL — the security-sensitive
> checklist in Step 2 is mandatory. If anything in "STOP conditions"
> occurs, stop and report. When done, update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat b43a7ca..HEAD -- src-tauri/src/settings.rs src-tauri/build.rs src-tauri/capabilities/settings.json src/settings/SettingsApp.tsx`
> On any change, compare excerpts below; mismatch = STOP.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: MED (ACL surface change — bounded by the repo's existing gating pattern)
- **Depends on**: none. NOTE: the earlier session's plan 002 (animation
  previews) also edits SettingsApp.tsx — reconcile textually if it landed.
- **Category**: tech-debt
- **Planned at**: commit `d40445e`, 2026-07-17; **refreshed 2026-07-18** at
  `b43a7ca` (build.rs now has six commands — `send_test_notification` and
  `set_appearance` landed via other plans; Config grew an `appearance`
  field, which `get_default_config` will serve automatically; SettingsApp
  line references updated); **review-plan pass 2026-07-18**: corrected
  the `get_config` excerpt (managed state became `StdMutex<Config>` in
  v5.1 — the earlier refresh missed it and it would have false-tripped
  the drift STOP), recorded that a "Reset to defaults" frontend test
  already exists (update, don't duplicate), fixed stale counts
  ("four" commands → six, "9" frontend tests → 11). In-scope files have
  ZERO drift `b43a7ca..HEAD` as of this pass — plans 010/011 landing
  touched only poller/rss/docs files.

## Why this matters

Config defaults are stated three times: `config.rs`'s `default_*`
functions (authoritative), `settings.rs::validate`'s ranges, and the
settings frontend's hand-maintained mirror — which self-documents the
problem:

```ts
// src/settings/SettingsApp.tsx:71-73
// Keep this synchronized with src-tauri/src/config.rs::Config::default.
// This frontend mirror can drift when Rust defaults change, so update both together.
export const DEFAULTS: Config = { … ~30 lines, incl. an appearance entry at line 101 … };
```

Every new config field pays a three-file lockstep tax (the v6 fields each
paid it, and the v5.1 `appearance` field just paid it again), and drift
fails soft (the "Reset to defaults" button silently applies stale values).
Since `Config: Serialize + Default` already holds (the `get_config`
command serializes it), a `get_default_config` command deletes the worst
copy for ~15 lines of rust. The `min=`/`max=` props duplicating `validate`
ranges are left in place — they're advisory UX, enforcement stays
server-side; a bounds-map export is not worth the plumbing (recorded as
rejected in the plans index).

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

`src-tauri/build.rs` (complete file — six commands as of `b43a7ca`):

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
            "send_test_notification",
            "set_appearance",
        ]),
    ))
    .expect("failed to run tauri-build");
}
```

`src-tauri/src/settings.rs:554-562` — the pattern to copy. Note the
managed state is `StdMutex<Config>` (v5.1's `set_appearance` mutates it);
`get_default_config` needs no state parameter at all, so it is simpler
than this:

```rust
#[tauri::command]
pub fn get_config(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, StdMutex<Config>>,
) -> Result<Config, String> {
    ensure_settings_window(&window)?;
    let config = state.inner().lock().unwrap().clone();
    Ok(config)
}
```

`ensure_settings_window` (same file, ~line 433) is the label gate; it has
its own test (`ensure_settings_window_gates_on_the_window_label`,
~line 1289 — it builds real windows via `tauri::test::mock_app()` +
`tauri::WebviewWindowBuilder` and asserts the `main` window is refused;
copy that shape for the new command's gate assertion).
`capabilities/settings.json` grants the six existing
`allow-<command>` permissions to `windows: ["settings"]` — read it to
copy the exact permission-string format. The `generate_handler![...]`
list is in `src-tauri/src/lib.rs` (~line 166).

Frontend: `SettingsApp.tsx` uses `DEFAULTS` in `resetDefaults()`
(`applyForm(DEFAULTS)`, line ~1110) — find all uses with
`rg -n "DEFAULTS" src/settings/`. The invoke pattern to copy is wherever
`invoke("get_config")` is called (top of the component's load effect).
Frontend tests (`src/settings/SettingsApp.test.tsx`, 11 tests as of this
refresh) mock IPC via the shared `mockLoads()` helper (~line 42) —
`get_default_config` needs an arm there next to `get_config`'s. NOTE: a
"Reset to defaults" test ALREADY EXISTS —
`it("Reset to defaults applies the Rust Config defaults mirror")`,
~line 214, asserting concrete default values (port 9789, ttl 8, tier
cap 50, unchecked start-paused, the rotation order) — Step 3 updates it
rather than adding a duplicate. The neighboring
`it("Reset restores the values returned by get_config")` (~line 201)
covers the *other* footer button ("Reset", not "Reset to defaults") and
needs no change.

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
corresponding `allow-get-default-config` permission to
`capabilities/settings.json` (copy the exact naming convention of the
existing six — read the file; tauri autogenerates the permission from
the command name); add the fn to `generate_handler![...]` in lib.rs.

Add a rust test mirroring
`ensure_settings_window_gates_on_the_window_label`: the label gate
rejects a `main`-labeled window, and the settings-window call returns a
value equal to `Config::default()` — `Config` already derives
`PartialEq` (`config.rs:7`), so a direct
`assert_eq!(returned, Config::default())` works; no serde detour needed.

**Verify**: `cargo test` → all pass incl. the new test; `cargo clippy --all-targets -- -D warnings` → exit 0. `git diff --stat src-tauri/capabilities/default.json` → empty (run this one from the repo root — from inside `src-tauri/` that path doesn't resolve and git errors with `fatal: ambiguous argument`).

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

In `SettingsApp.test.tsx`: add the `get_default_config` arm to the
shared `mockLoads()` helper (~line 42), returning a fixture that carries
the Rust defaults — the existing reset test asserts concrete values
(port 9789, ttl 8, tier cap 50), so the fixture must match them. Then
UPDATE the existing
`it("Reset to defaults applies the Rust Config defaults mirror")`
(~line 214) — do NOT add a duplicate: it already loads non-default
values (port `4321` fixture) and clicks "Reset to defaults". It should
keep passing once the reset path reads the invoked defaults (the click
may now need an `await`/`findBy` if the defaults arrive async — model on
how the test's own `findByLabelText` calls already wait); rename it to
drop the word "mirror" since the mirror is gone.

**Verify**: `npx vitest run` → all pass incl. the reset test; `npx tsc --noEmit` → exit 0 (proves no dangling DEFAULTS references); `npx vite build` → exit 0.

### Step 4: Docs

`docs/V5_TECHNICAL_SPEC.md` §2: add `get_default_config` to the command
table (read-only, no side effects, settings-window-gated).

`docs/TESTING_STRATEGY.md` §0: in the rust row, bump the `settings`
sub-count by the number of new `#[test]` fns Step 1 actually added
(likely 1 — one fn asserting both the gate and the value, per the repo's
table-driven convention) AND move the row's leading total by the same
delta — the sub-counts must keep summing to the total, and §0 is the
only place counts live (per `CLAUDE.md`/`AGENTS.md`). The frontend row
changes ONLY if you added a new `it()` block; Step 3 updates an existing
test and adds a mock arm, which adds zero, so the frontend counts likely
stay put. Re-read §0 at execution time before editing — concurrent
plans move it (at this refresh it read `232 tests — settings 38, …` and
`62 tests — … settings form 11 …`).

**Verify**: full gates green.

### Step 5: Manual ACL smoke (operator, once)

Same discipline as the v5 exit criteria: from the *main* window's
devtools console, `window.__TAURI__.core.invoke("get_default_config")`
must be DENIED; from the settings window it succeeds. (Requires a dev
build; hand to operator if not runnable.)

**Verify**: operator confirmation or explicitly reported pending.

## Test plan

- Rust: 1 test fn (label gate rejects `main` + returned value equals
  `Config::default()`) modeled on
  `ensure_settings_window_gates_on_the_window_label` (settings.rs ~1289).
- Frontend: 1 mock arm in `mockLoads()` + UPDATE the existing
  reset-to-defaults test (see Step 3) — no new `it()` expected.

## Done criteria

- [ ] `grep -c "const DEFAULTS" src/settings/SettingsApp.tsx` → 0
- [ ] Step 2's four-leg checklist reported complete
- [ ] `git diff b43a7ca..HEAD -- src-tauri/capabilities/default.json` → empty
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
