# Plan 089: Notification history — Settings History section + invoke commands

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat <planned-at SHA>..HEAD -- src-tauri/src/history.rs src-tauri/src/settings.rs src-tauri/build.rs src-tauri/capabilities/settings.json src/settings/SettingsApp.tsx`
> If any changed since this plan was written, compare the "Current state"
> excerpts against the live files before proceeding; on a mismatch, treat it
> as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: MED — adds two `#[tauri::command]`s. The `build.rs` opt-in is
  the load-bearing security step (see "The one thing that must not go
  wrong"); getting it wrong silently exposes commands to the overlay
  window.
- **Depends on**: **plan 088 (must be DONE and merged)** — this plan calls
  `HistoryStore::read_recent` / `clear`, which 088 creates. Do not start
  until `src-tauri/src/history.rs` exists on the branch you build from.
- **Category**: direction
- **Planned at**: commit `<fill at dispatch — re-stamp after 088 merges>`
- **Note**: this plan was filed alongside 088 before 088 executed. It
  **requires a review-plan pass** to re-stamp its SHA and re-verify every
  citation against 088's actual shipped API before it is dispatched.

## Why this matters

Plan 088 built the history store and the write hook, but shipped it dark:
`history_enabled` defaults to `false`, and even switched on, nothing in the
app can display what it records. This plan closes the loop the operator
originally asked for — *"do we hold past notification data? can I look it up
in the control panel?"* — by adding a History section to the Settings window
and the toggle that turns recording on.

The split mirrors plan 083 (football backend) / plan 084 (football display),
the same two-plan shape this repo already used successfully for a
backend-then-UI feature.

## The one thing that must not go wrong

Tauri v2 grants app-defined commands to **every window by default**. This
repo defeats that with a deny-by-default opt-in in `src-tauri/build.rs` plus
a window-scoped `capabilities/settings.json`. CLAUDE.md states the rule
directly:

> never add a new `#[tauri::command]` without adding it to that `build.rs`
> list — otherwise it silently becomes callable from the overlay (`main`)
> window too, breaking the receive-only guarantee.
> `capabilities/default.json` must never change.

A `get_history` command callable from the overlay would let the receive-only
overlay read every persisted notification. Both new commands must appear in
**both** files, and `capabilities/default.json` must stay byte-identical.

## Current state

### `src-tauri/build.rs` (the whole file — 9 commands today)

```rust
fn main() {
    // v5 (V5_TECHNICAL_SPEC.md §2): tauri allows app-defined commands to
    // EVERY window by default — this opt-in flips them to deny-by-default
    // so capabilities/settings.json can grant them to the settings window
    // alone, keeping the overlay (`main`) receive-only. never add a
    // #[tauri::command] to generate_handler without also listing it here.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_config",
            "get_connector_health",
            "get_default_config",
            "get_recent_log_lines",
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

### `src-tauri/capabilities/settings.json` (the whole file)

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "settings",
  "description": "Settings window only: the nine v5 settings commands plus event listen (V5_TECHNICAL_SPEC.md §2). The overlay's default.json must never gain any of these.",
  "windows": ["settings"],
  "permissions": [
    "allow-get-config",
    "allow-get-connector-health",
    "allow-get-default-config",
    "allow-get-recent-log-lines",
    "allow-get-secret-status",
    "allow-save-config-and-relaunch",
    "allow-set-secret",
    "allow-send-test-notification",
    "allow-set-appearance",
    "core:event:allow-listen",
    "core:event:allow-unlisten"
  ]
}
```

Note the `description` says "nine" — update it to eleven.

### The command exemplar (`src-tauri/src/settings.rs:790-798`)

Every invoke command starts with the window scope check. Match this shape:

```rust
#[tauri::command]
pub async fn get_recent_log_lines(window: tauri::WebviewWindow) -> Result<Vec<String>, String> {
    ensure_settings_window(&window)?;
    // plan 077: no content-based redaction layer here on purpose —
    // notifier.rs's token redaction (plan 006, `e.without_url()`) already
    // keeps secrets out of the log file itself, so the file is safe to
    // surface read-only in the settings window.
    crate::logging::read_recent_lines(200).map_err(|e| e.to_string())
}
```

Commands live under this banner comment (`settings.rs:511-514`), which
records that they are deliberately untested — the logic they call is tested
instead:

```rust
// ---------------------------------------------------------------------------
// invoke commands (thin; scope-checked; untested by design — the logic
// they call is tested above, `TESTING_STRATEGY.md` §4.11)
// ---------------------------------------------------------------------------
```

The config-dir helper is `notchtap_config_dir()` (`settings.rs:505-509`):

```rust
fn notchtap_config_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|h| Config::dir_from_home(&h))
        .ok_or_else(|| "could not determine home directory".to_string())
}
```

### The UI exemplar (`src/settings/SettingsApp.tsx:1140-1190`)

`DiagnosticsSection` is the closest analogue — a read-only fetch-on-mount
list with a refresh control. Copy its shape, including the deliberate
mount-only `useEffect` and its biome-ignore justification:

```tsx
function DiagnosticsSection() {
  const [logLines, setLogLines] = useState<string[] | null>(null);

  function refresh() {
    invoke<string[]>("get_recent_log_lines")
      .then((fetched) => setLogLines(fetched))
      .catch(() => {
        // advisory fetch — a failed read leaves the previous lines shown
      });
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-only fetch on section-open — refresh is re-created every render, so adding it would re-invoke get_recent_log_lines on every render.
  useEffect(() => {
    refresh();
  }, []);

  const logText =
    logLines === null
      ? "Loading…"
      : logLines.length === 0
        ? "No log lines yet."
        : logLines.join("\n");

  return (
    <SettingsGroup
      title="Recent log lines"
      description="The last 200 lines of ~/Library/Logs/notchtap/notchtap.log. Read-only; rotated backups are available via Console.app."
    >
      {/* ... */}
    </SettingsGroup>
  );
}
```

### The sidebar (`src/settings/SettingsApp.tsx:95-120`)

`SectionId` is a nine-member union ending in `"diagnostics"`, with a
matching entry in the nav array (`{ id: "diagnostics", label: "Diagnostics",
icon: ScrollText }`), a copy record at `:164`, and a render branch at
`:1690`. A tenth section follows the same four touch points. Icons come from
`lucide-react` — pick an existing one (e.g. `History` or `Clock`).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass |
| Rust lint | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Rust format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Frontend tests | `npx vitest run` | all pass, +N new |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Frontend lint | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/settings.rs` (two new commands + their DTO)
- `src-tauri/build.rs` (two list entries)
- `src-tauri/capabilities/settings.json` (two permissions + description)
- `src-tauri/src/lib.rs` (`generate_handler!` registration only)
- `src/settings/SettingsApp.tsx` (History section, sidebar entry, the
  `history_enabled` toggle in the General group)
- `src/settings/settings.css` (only if the section needs new rules)
- `src/settings/SettingsApp.test.tsx` (new tests)
- `docs/TESTING_STRATEGY.md` (§0 counts)

**Out of scope**:
- `src-tauri/capabilities/default.json` — must stay byte-identical. This is
  a STOP condition, not a preference.
- `src-tauri/src/history.rs` — plan 088's module; consume its API, do not
  change it. If you need a signature it does not expose, STOP and report
  rather than widening it.
- `src-tauri/src/engine.rs`, `queue.rs` — no ingest or queue change.
- The overlay (`src/App.tsx`, `src/components/**`, `src/styles.css`) —
  history is a Settings-window feature only.

## Steps

### Step 1: Add the two commands in `settings.rs`

Define a serializable DTO rather than leaking `HistoryEntry` directly if the
frontend needs a flatter shape; otherwise return
`Vec<crate::history::HistoryEntry>` (it already derives `Serialize`).

```rust
#[tauri::command]
pub async fn get_history(window: tauri::WebviewWindow) -> Result<Vec<HistoryEntry>, String> {
    ensure_settings_window(&window)?;
    let dir = notchtap_config_dir()?;
    let store = crate::history::HistoryStore::new(dir).map_err(|e| e.to_string())?;
    store.read_recent(200).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(window: tauri::WebviewWindow) -> Result<(), String> {
    ensure_settings_window(&window)?;
    let dir = notchtap_config_dir()?;
    let store = crate::history::HistoryStore::new(dir).map_err(|e| e.to_string())?;
    store.clear().map_err(|e| e.to_string())
}
```

Place both under the existing invoke-commands banner, alongside
`get_recent_log_lines`. Register both in `lib.rs`'s `generate_handler!`.

**Note the ordering contract**: 088's `read_recent` returns oldest → newest.
The UI reverses for display; do not change the rust contract.

**Verify**: `cd src-tauri && cargo build --locked` → exit 0.

### Step 2: The security opt-in — `build.rs` + `capabilities/settings.json`

Add `"clear_history"` and `"get_history"` to `build.rs`'s command list
(keep it alphabetical: `clear_history` sorts first, `get_history` between
`get_default_config` and `get_recent_log_lines`).

Add `"allow-clear-history"` and `"allow-get-history"` to
`capabilities/settings.json`'s permissions, and change "the nine v5 settings
commands" to "the eleven".

**Verify**:
- `grep -c "history" src-tauri/build.rs` → **2**
- `grep -c "history" src-tauri/capabilities/settings.json` → **2**
- `git diff --stat -- src-tauri/capabilities/default.json` → **empty**
- `cd src-tauri && cargo build --locked` → exit 0 (the generated ACL
  schema regenerates; a mismatch fails the build here, which is the point)

### Step 3: The `history_enabled` toggle

Add a toggle to the **General** group in `SettingsApp.tsx` bound to 088's
`history_enabled` config field, saved through the existing
`save_config_and_relaunch` path like every other config field.

Help copy must be honest about two things: it records notification
**content** to `~/.config/notchtap/history.jsonl`, including cmux payloads;
and whether it applies immediately or after relaunch depends on the save
path — if it routes through `save_config_and_relaunch` (which calls
`app.restart()`), say "Applies after Save & Relaunch", following plan 085's
`resting_state` precedent for exactly this situation.

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 4: The History section

Add the tenth sidebar section (four touch points: `SectionId` union, nav
array, copy record, render branch) and a `HistorySection` component modelled
on `DiagnosticsSection`.

Requirements:
- Fetch on mount via `invoke<HistoryEntry[]>("get_history")`, same
  advisory-catch pattern (a failed read leaves previous entries shown).
- Display **newest first** — reverse the rust ordering at the display layer.
- Each row: recorded time (from `recorded_at_ms`), source/origin, title, and
  body. Keep it scannable; this is a list, not a card renderer — do NOT
  import overlay components or `presentation.ts`.
- Empty state: distinct copy for "history is off" (when `history_enabled` is
  false) versus "on, but nothing recorded yet". These are different problems
  and the user needs to be able to tell them apart.
- A "Clear history" control calling `clear_history`, then refetching. Given
  it is destructive and irreversible, require a confirmation step —
  **but do NOT use `window.confirm`** or any browser modal dialog; use an
  in-component two-step (click → "Really clear?" → click) so nothing blocks
  the webview.

**Verify**: `npx tsc --noEmit` and `npx biome ci .` → exit 0.

### Step 5: Tests

In `src/settings/SettingsApp.test.tsx`, following its existing
`invoke`-mocking pattern:
1. History section renders entries newest-first from a mocked `get_history`.
2. Empty-history state renders the "nothing recorded yet" copy.
3. History-disabled state renders the distinct "history is off" copy.
4. The clear control requires the second confirming click before
   `clear_history` is invoked (assert it is NOT called on the first click).
5. The `history_enabled` toggle round-trips into the saved config payload.

**Verify**: `npx vitest run` → all pass, +5.

### Step 6: Docs + full gate run

Update `docs/TESTING_STRATEGY.md` §0's frontend row with the live count.
Then run every gate: `cargo test --locked`, `cargo clippy --locked
--all-targets -- -D warnings`, `cargo fmt --check`, `npx vitest run`,
`npx tsc --noEmit`, `npx biome ci .`, `npx vite build`.

**Verify**: all exit 0.

## Test plan

5 new frontend tests (above), modelled on the existing
`SettingsApp.test.tsx` cases. No new rust tests: the two commands are thin
scope-checked wrappers, matching the "untested by design" banner at
`settings.rs:511-514` — 088 already tests `read_recent`/`clear` directly.

## Done criteria

ALL must hold:

- [ ] `cargo test --locked`, `cargo clippy --locked --all-targets -D warnings`,
      `cargo fmt --check` all exit 0
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`,
      `npx vite build` all exit 0
- [ ] `grep -c "history" src-tauri/build.rs` returns **2**
- [ ] `grep -c "history" src-tauri/capabilities/settings.json` returns **2**
- [ ] `git diff -- src-tauri/capabilities/default.json` is **empty**
- [ ] `git diff --stat -- src/App.tsx src/components/ src/styles.css` is
      **empty** (overlay untouched)
- [ ] Toggling history off and reopening the section shows the
      "history is off" copy, not an empty list — covered by test 3
- [ ] `plans/README.md` status row for 089 updated

## STOP conditions

- Plan 088 is not merged / `src-tauri/src/history.rs` does not exist.
- 088's shipped API differs from this plan's excerpts (it was filed before
  088 executed) — re-read `history.rs` and STOP if `read_recent`/`clear`
  have different signatures or ordering than described.
- Any change to `capabilities/default.json` appears necessary.
- The build fails with an ACL/permission error after Step 2 — that means
  the command names in `build.rs` and the `allow-*` permissions disagree;
  report the exact mismatch rather than guessing at the naming convention.
- You need a browser modal (`confirm`/`alert`) to implement the clear
  confirmation — use an in-component two-step instead.

## Maintenance notes

- **Every future `#[tauri::command]` repeats this dance**: `build.rs` list +
  `capabilities/settings.json` permission + `generate_handler!`. Three
  places, and only the build catches a mismatch.
- The Settings window reads history through a **freshly constructed**
  `HistoryStore` rather than sharing the Engine's — deliberate: the store is
  stateless apart from its path, and the settings commands must work even
  when `history_enabled` is off (so the Engine holds `None`). If a future
  change gives the store real in-memory state, this must be revisited.
- What a reviewer should scrutinize: both files updated in Step 2,
  `default.json` untouched, the two-step clear confirmation actually
  gating the invoke, and that the newest-first reversal happens in the UI
  rather than by changing 088's contract.
