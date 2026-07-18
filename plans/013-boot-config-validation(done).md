# Plan 013: Run `validate()` at boot so hand-edited configs get the same contract as the settings window

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 6a7fd5a..HEAD -- src-tauri/src/config.rs src-tauri/src/settings.rs src-tauri/src/lib.rs`
> On any change, compare excerpts below; mismatch = STOP.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: MED (changes boot behavior for existing bad configs — mitigated by warn-don't-die design below)
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `d40445e`, 2026-07-17; drift baseline refreshed to `6a7fd5a` 2026-07-18 (excerpts re-verified against live code — `Config::load` body unchanged, `validate` signature unchanged; only line numbers moved, updated below)

## Why this matters

The config file is a documented editing surface (hand-editing predates the
settings window and remains supported — `detect_path` is file-only by
design). But the two write paths enforce different contracts: the settings
window runs `settings::validate()` before saving; `Config::load()` at boot
parses and returns with **zero range validation**. Concretely:

- `default_ttl = 0` boots fine, then every notification rotates out on
  the first 250 ms heartbeat tick — near-invisible cards with no error
  anywhere.
- `max_queued_per_tier = 0` makes every push beyond the visible one
  return 429.
- An `espn_leagues` entry with whitespace/emptiness (which `validate`
  rejects) gets spliced into the ESPN URL path unvalidated.

Design decision for this plan: **validate-and-warn, not fail-fast.** The
repo's fail-fast rule covers *malformed TOML* (unparseable file → exit
with a clear error). A parseable-but-out-of-range value should not brick
an always-on app at login; instead, boot logs every violation loudly and
continues with the file's values (the queue/pollers already clamp the
worst cases locally). This keeps boot deterministic while making the
silent-degradation failure mode loud. If the maintainer prefers hard
fail-fast, that is a one-line change flagged in Maintenance notes.

## Current state

`src-tauri/src/config.rs:267-283` — `Config::load()`:

```rust
pub fn load() -> anyhow::Result<Self> {
    // spec §9 pins ~/.config/notchtap/config.toml. …
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
    let path = Self::dir_from_home(&home).join("config.toml");

    if !path.exists() {
        return Ok(Self::default());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("failed to read config at {:?}: {}", path, e))?;
    Self::parse(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse config at {:?}: {}", path, e))
}
```

`src-tauri/src/settings.rs:35+` — `pub fn validate(c: &Config) ->
Result<(), Vec<String>>`: per-field range checks (port ≥1024,
default_ttl 1–3600, max_queued_per_tier 1–1000, espn_poll_secs 5–3600,
league slug shape, rss feed URL parse, rotation_order permutation, etc.),
returning the full message list. It is called today only from
`save_config_and_relaunch` (`settings.rs:596`).

Two other validators exist in `settings.rs` — neither changes this plan:
`validate_appearance` is already called *inside* `validate()`
(`settings.rs:137`), so the one boot call below covers appearance ranges
too; `validate_secret_value` (`settings.rs:296`) guards the secrets write
path and has no boot-time role. Do not add separate calls for either.

Call-site of `Config::load()`: `src-tauri/src/lib.rs:79`, inside `run()`.
Verified ordering as of the drift baseline: `logging::init_logging()` runs
first (`lib.rs:70`), so warnings emitted right after the load already reach
the file log. (If a future reorder moves config load before logging init,
fall back to placing the validate call after logging init, operating on the
already-loaded value.)

Module-boundary note: `config.rs` must not import from `settings.rs` if
that creates a cycle — check imports first (`settings.rs` already
`use`s `config::Config`). Therefore the validate call goes at the
**call-site in `lib.rs`**, not inside `Config::load()` — this also keeps
`Config::load()`'s contract ("parse or die") unchanged.

Conventions: thiserror in modules / anyhow at boundary; `tracing::warn!`
for degraded-but-running conditions. Counts live ONLY in
`docs/TESTING_STRATEGY.md` §0.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Tests | `cargo test` (from `src-tauri/`) | all pass |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` (from `src-tauri/`) | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/lib.rs` (the `Config::load()` call-site — a few lines)
- `src-tauri/src/settings.rs` ONLY if `validate` needs `pub` visibility
  adjustments (it is already `pub`)
- `docs/ARCHITECTURE.md` §10 — one sentence documenting the boot-time
  validate-and-warn behavior
- `docs/TESTING_STRATEGY.md` §0 if tests are added
- `plans/README.md` (status row)

**Out of scope**:
- `validate()`'s rules themselves — no new rules, no changed ranges.
- `Config::parse` / the back-compat inheritance shims.
- Percent-encoding the league slug in `poller.rs` (validate's slug rule
  covers the practical gap; deeper URL-encoding is deferred).
- The settings-window save path.

## Git workflow

- Current branch; commit style: `boot: run settings::validate on the loaded config — warn loudly, never silently degrade`.
- Do NOT push.

## Steps

### Step 1: Validate at the boot call-site

In `lib.rs`'s `run()`, immediately after the successful `Config::load()`
match (`lib.rs:79` at the drift baseline — logging init already precedes
it, see Current state):

```rust
// Boot-time contract parity with the settings window (plan 013): the
// file is the other editing surface, so it gets the same validation —
// but warn-and-continue, not exit: a range violation must not brick an
// always-on login item. Malformed TOML still fails fast in Config::load.
if let Err(violations) = crate::settings::validate(&config) {
    for v in &violations {
        tracing::warn!(violation = %v, "config.toml value out of range — running with it anyway");
    }
}
```

**Verify**: `cargo build` from `src-tauri/` → exit 0; `cargo clippy --all-targets -- -D warnings` → exit 0.

### Step 2: Test it

`validate` is already exhaustively tested in `settings.rs`. What needs a
test is the *wiring* — that boot tolerates an invalid-range config. If
`run()` is untestable directly (it is — tauri setup), add instead a small
pure seam ONLY if one already suggests itself; otherwise rely on:
1. the existing `settings::validate` suite (rules), and
2. a manual smoke **against a scratch HOME, never the real config**.
   `Config::load()` resolves the path via `dirs::home_dir()`, which honors
   `$HOME`, so:
   ```sh
   SCRATCH=$(mktemp -d)
   mkdir -p "$SCRATCH/.config/notchtap"
   printf 'default_ttl = 0\n' > "$SCRATCH/.config/notchtap/config.toml"
   HOME="$SCRATCH" npm run tauri dev
   ```
   Confirm the console shows the warn line ("config.toml value out of
   range") and the app still boots. Side effects (logs, disabled telegram
   from missing secrets) land in the scratch HOME and are expected. Quit,
   then `rm -rf "$SCRATCH"`. Caveat: if another notchtap instance already
   holds port 9789, the smoke app exits with a bind error shortly *after*
   boot — the warn line appearing before that is still a passing smoke;
   don't misread the port conflict as a boot failure. Only a human
   operator should ever exercise the variant that edits the real
   `~/.config/notchtap/config.toml`.

Document in your report which form you used. Do NOT build a test harness
for `run()` — that's out of proportion (the repo's §5.1 records lib.rs
wiring as thin-by-design).

**Verify**: `cargo test` → all pass; manual smoke output quoted in your report (or explicitly handed to operator).

### Step 3: Document

`docs/ARCHITECTURE.md` §10: after the "read once at startup" paragraph,
add one sentence: boot validates the loaded config with the settings
window's `validate()` and logs each violation as a warning, continuing
with the file's values; malformed TOML still fails fast.

**Verify**: `grep -c "warn" docs/ARCHITECTURE.md` → ≥1 in §10's area (eyeball
the diff). NOTE: this file already contains one unrelated "warn" at line
~366 (nothing to do with §10), so a plain `grep -c "warn"` reads `1`
*before* this step even runs — it cannot tell you whether your sentence
landed. Use `grep -A3 "reads this file once at startup" docs/ARCHITECTURE.md`
instead and confirm your new sentence appears in that context, or grep for
a distinctive word from the sentence you actually wrote.

## Test plan

- No new unit tests required (rules already covered by the settings suite
  — see `docs/TESTING_STRATEGY.md` §0 for the current count, it has moved
  since this plan was written); the manual smoke in Step 2 is the wiring
  proof. If you do add a seam + test, update §0.

## Done criteria

- [ ] `grep -c "settings::validate" src-tauri/src/lib.rs` → 1
- [ ] `cargo test` exits 0; clippy/fmt gates exit 0
- [ ] ARCHITECTURE.md §10 sentence added
- [ ] Manual smoke done or handed to operator (stated in report)
- [ ] No files outside scope modified
- [ ] `plans/README.md` status row updated

## STOP conditions

- `Config::load()` happens before logging init AND moving the validate
  call after logging init would separate it from other config-derived
  setup in a way that needs restructuring — report the ordering instead
  of restructuring `run()`.
- Importing `settings` from the `lib.rs` position creates any module
  cycle (it shouldn't — both are top-level modules of the same crate).
- The maintainer's docs turn out to already record a "no boot validation,
  ever" decision you find during Step 3's ARCHITECTURE read — surface it.

## Maintenance notes

- If the maintainer later prefers hard fail-fast for range violations,
  replace the warn loop with
  `anyhow::bail!("config.toml invalid:\n{}", violations.join("\n"))` —
  the tradeoff (a typo bricks the login item until hand-fixed) is why
  this plan chose warn.
- Plan 021 adds a port pre-flight to the settings save path — same theme
  (the two surfaces converging on one contract), no code overlap.
- New `validate` rules automatically apply to boot now — rule authors
  should keep messages self-contained since they surface in the log.
