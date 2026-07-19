# Plan 045: bump the stale `tauri-nspanel` git pin past two crash/segfault fixes

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in "STOP conditions" occurs, stop and
> report — do not improvise. This plan requires network access (to fetch
> the updated git dependency) and a macOS machine with the Rust
> toolchain (`tauri-nspanel` is a `target_os = "macos"`-only dependency,
> and this repo's own convention, per `CLAUDE.md`, is that rust
> build/test on this dependency happens on the mac dev machine, not in a
> Linux CI sandbox). When done, update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `grep -n "tauri-nspanel" src-tauri/Cargo.toml src-tauri/Cargo.lock`
> Confirm the pinned rev in `Cargo.toml` is still
> `18ffb9a201fbf6fedfaa382fd4b92315ea30ab1a` and `Cargo.lock` still shows
> `version = "2.0.1"` at that same rev. If the pin has already moved,
> re-derive the target rev (Step 1) instead of assuming this plan's
> numbers still apply — dependency history moves independently of this
> repo.

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: MED — 39 upstream commits sit between the current pin and
  the proposed target, including a rename and a macro-signature change;
  a naive bump can hit a compile break that needs a source-level fixup,
  not just a lockfile change.
- **Depends on**: none
- **Category**: dependency
- **Planned at**: commit `f2cbae6`, 2026-07-19 — dependency facts below
  verified live against `github.com/ahkohd/tauri-nspanel` on that date;
  **re-verify the target rev is still current before executing**, since
  upstream may have moved further by execution time.

## Why this matters

`src-tauri/Cargo.toml:37` pins `tauri-nspanel` to
`rev = "18ffb9a201fbf6fedfaa382fd4b92315ea30ab1a"` — the tip of that
project's old `v2` branch, dated 2025-06-15. This is the exact mechanism
this app relies on for its one core platform trick: converting the
overlay's plain `NSWindow` into a nonactivating `NSPanel` so it survives
Space switches and floats over fullscreen apps without ever stealing
focus (`src-tauri/src/lib.rs:222-231`, `apply_overlay_native_config`).

Independently confirmed (via a live fetch of the upstream repository,
2026-07-19): the repository's current `v2.1` branch tip,
`a3122e894383aa068ec5365a42994e3ac94ba1b6` (dated 2026-03-15), is a
straight fast-forward 39 commits ahead of the pinned rev. Among those 39
commits, at minimum three touch the exact panel-conversion/lifecycle
code path this app depends on: a segfault fix, a fix for a crash when
`panel.close()` is called, and a fix for an issue where `panel.close()`
does not free the webview. This app's own usage
(`lib.rs:224-230`: `to_panel()` then a manual `set_style_mask` call, no
`close()` call anywhere) is not known to trigger the `close()`-specific
fixes today, but staying 13 months behind on a native macOS FFI wrapper
this central to the app's only distinguishing platform behavior means
any future change that touches panel lifecycle inherits already-fixed
upstream bugs, and the gap only widens with time. This question was
explicitly deferred twice before (`plans/README.md`'s "Findings
considered and rejected" — "needs network," recorded in both the deep
session and the third audit session); this plan answers it with a
concrete action instead of deferring a third time.

## Current state

- `src-tauri/Cargo.toml:37` (inside the `[target.'cfg(target_os =
  "macos")'.dependencies]` section):

  ```toml
  tauri-nspanel = { git = "https://github.com/ahkohd/tauri-nspanel", rev = "18ffb9a201fbf6fedfaa382fd4b92315ea30ab1a" }
  ```

- `src-tauri/Cargo.lock:4047-4049`:

  ```
  name = "tauri-nspanel"
  version = "2.0.1"
  source = "git+https://github.com/ahkohd/tauri-nspanel?rev=18ffb9a201fbf6fedfaa382fd4b92315ea30ab1a#18ffb9a201fbf6fedfaa382fd4b92315ea30ab1a"
  ```

- The only production use of the crate, `src-tauri/src/lib.rs:222-231`:

  ```rust
  #[cfg(target_os = "macos")]
  {
      use tauri_nspanel::WebviewWindowExt as _;
      let panel = window
          .to_panel()
          .map_err(|e| format!("nspanel conversion failed: {e:?}"))?;
      // NSWindowStyleMaskNonactivatingPanel (1 << 7); the window
      // is borderless (mask 0), so the panel bit is the whole mask.
      panel.set_style_mask(1 << 7);
  }
  ```

  This is the ONLY call into `tauri_nspanel` in the whole codebase
  (confirm with `grep -rn "tauri_nspanel\|nspanel" src-tauri/src`) — the
  surface a rename or macro change could hit is small and localized.

- Proposed target: `github.com/ahkohd/tauri-nspanel`'s `v2.1` branch tip,
  `a3122e894383aa068ec5365a42994e3ac94ba1b6` — **re-verify this is still
  the branch tip at execution time** (Step 1), since it may have moved
  since 2026-07-19.

## Commands you will need

| Purpose | Command (from `src-tauri/`) | Expected on success |
|---|---|---|
| Build | `cargo build --locked` | exit 0, no warnings |
| Rust tests | `cargo test --locked` | all pass |
| Lint/format | `cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Full local gate | `just test-all` (from repo root, if `just` is installed — `brew install just` first if not) | all gates pass |

## Scope

**In scope** (the only files you should modify):
- `src-tauri/Cargo.toml` — the `tauri-nspanel` dependency's `rev`.
- `src-tauri/Cargo.lock` — regenerated by `cargo update`/`cargo build`,
  not hand-edited.
- `src-tauri/src/lib.rs` — ONLY if the bump surfaces a compile break in
  the one call site quoted above (a rename or signature change); no
  other file in `lib.rs` should be touched.

**Out of scope** (do NOT touch, even though they look related):
- Any other dependency in `Cargo.toml`/`Cargo.lock` — this plan is
  `tauri-nspanel` only.
- `src-tauri/src/lib.rs`'s window-positioning, tray, or hotkey code
  beyond the one panel-conversion block, if the bump requires no change
  there at all — resist the urge to "clean up while in there."
- Adding a `panel.close()` call or any new use of the crate's API — this
  plan bumps the dependency, it does not adopt new functionality from
  it.

## Git workflow

- Branch: `advisor/045-bump-tauri-nspanel-pin` (or work directly if the
  operator dispatched you that way).
- Commit style (from `git log`): lowercase `area: imperative summary`,
  e.g. `deps: bump tauri-nspanel pin past segfault/close() fixes`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 0: Re-confirm the target rev and read the intervening changes

Do not skip this even though the research below was already done once.
Dependency state is external to this repo and can move between when
this plan was written and when it's executed.

1. Fetch `https://github.com/ahkohd/tauri-nspanel/commits/v2.1` (or
   equivalent — `git ls-remote https://github.com/ahkohd/tauri-nspanel
   refs/heads/v2.1` if you have git+network access without a browser
   tool) and confirm the current tip SHA. If it's still
   `a3122e894383aa068ec5365a42994e3ac94ba1b6`, proceed with that target.
   If it has moved, use the new tip instead — the goal is "current
   `v2.1` tip," not this specific SHA.
2. Read the commit list between the pinned rev
   (`18ffb9a201fbf6fedfaa382fd4b92315ea30ab1a`) and your target tip.
   Specifically check for: a rename of `WebviewWindowExt`/`to_panel`/
   `set_style_mask` (the three symbols this app calls), or a
   macro/signature change to any of them. At the time this plan was
   written, the intervening history included commits titled
   `rename method` and `feat: support snake_case config in panel!
   macro` — confirm whether either affects the three symbols this app
   actually uses (it may not; `panel!` is a different, unused macro
   path).

**Verify**: you can state the exact target rev and whether any of the
three call-site symbols renamed. If you cannot reach the network to do
this, STOP (see STOP conditions) — do not guess at a rev.

### Step 1: Bump the pin

Edit `src-tauri/Cargo.toml:37` to point `rev` at the target from Step 0.

**Verify**: `grep -n "tauri-nspanel" src-tauri/Cargo.toml` shows the new
rev.

### Step 2: Regenerate the lockfile

From `src-tauri/`, run `cargo update -p tauri-nspanel` (this fetches the
new rev and updates only this crate's lock entry — not a full
`cargo update` of every dependency). Then confirm the lock is internally
consistent with `cargo build --locked`.

**Verify**: `grep -n "tauri-nspanel" src-tauri/Cargo.lock` shows the new
rev in both the `source` line and the trailing `#`-fragment; `cargo
build --locked` → exit 0.

If `cargo build --locked` fails with a compile error localized to
`lib.rs:222-231`'s three symbols, fix the call site to match the new
API (rename/signature change) — this is the one case where touching
`lib.rs` beyond that block is in scope, but keep the fix as small as the
actual API change requires. If the failure is anywhere else (a
transitive dependency conflict, an unrelated crate), STOP — that's a
sign the bump has a wider blast radius than this plan scoped for.

### Step 3: Full verification gate

Run the complete local gate mirroring CI.

**Verify**: `cargo test --locked` → all pass, no count change expected
(this is a dependency-only change; if a count differs, something
unexpected happened — investigate before proceeding). `cargo fmt
--check && cargo clippy --locked --all-targets -- -D warnings` → exit 0.

### Step 4: Manual smoke check (operator-owed, same discipline as other nspanel-adjacent plans)

This repo's convention (see plan 007's/plan 020's precedent) is that a
change touching the notch/hud overlay's platform behavior gets a manual
GUI smoke check on real hardware, which an executor in a worktree
usually cannot perform. Note in your final report (and in the
`plans/README.md` status row) that this step is owed to the operator:
launch the app (`npm run tauri dev`), confirm the overlay still floats
correctly over the notch/hud, survives a Spaces switch, and stays
visible over a fullscreen app (the exact behaviors
`apply_overlay_native_config` exists for) — do not mark this plan fully
verified without it, but do not block landing the code change on it
either, matching how plans 010/012/018/023 handled the same kind of
hardware-only gap.

## Test plan

No new automated tests are appropriate here — this is a dependency
version bump with no behavior change to notchtap's own code (unless
Step 2 required a call-site fix, in which case the existing test suite,
which exercises `apply_overlay_native_config` and window setup only
indirectly via integration-style tests, is the safety net; do not invent
a new unit test for a third-party crate's internals).

- Verification: `cargo test --locked` (from `src-tauri/`) → all pass, no
  count change from before this plan.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `src-tauri/Cargo.toml`'s `tauri-nspanel` rev matches the confirmed
      Step-0 target
- [ ] `cargo build --locked` exits 0
- [ ] `cargo test --locked` exits 0, same test count as before this plan
      (no count line change expected)
- [ ] `cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings`
      exits 0
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row updated, explicitly noting the manual
      overlay smoke check (Step 4) is operator-owed if you couldn't run
      it yourself

## STOP conditions

Stop and report back (do not improvise) if:

- You cannot reach the network to confirm the current `v2.1` tip (Step
  0) — do not guess at a rev or proceed with the plan's original one
  without re-confirming it's still current.
- `cargo build --locked` fails anywhere other than the three named
  call-site symbols in `lib.rs:222-231` — a wider break means the bump's
  blast radius is larger than this plan scoped for.
- The upstream repository shows evidence of being effectively abandoned
  or the `v2.1` branch itself looks unstable/experimental (e.g. marked
  WIP, or its own CI is red) rather than a maintained successor to `v2`
  — report the finding instead of bumping to something worse than the
  status quo.
- `cargo test --locked`'s count changes in a way this plan didn't
  predict (it should be zero test-count change) — investigate before
  reporting done; an unexplained count change on a dependency-only bump
  is a signal something unintended happened.

## Maintenance notes

- This closes out a question twice-deferred in `plans/README.md`'s
  "Findings considered and rejected" section for lack of network access
  — update that section's wording (or note in this plan's done-entry)
  that the question is now answered and the pin is current as of this
  plan's execution, so a future audit doesn't re-flag it as "still
  unchecked."
- The sibling dependency `smappservice-rs` (0.1.x, `src-tauri/Cargo.toml`)
  was checked in the same audit pass and found to have no comparable
  issue — small, single-purpose, feature-complete crate with no negative
  signal. No action needed there; do not fold a `smappservice-rs` bump
  into this plan.
- A reviewer should scrutinize: that the lockfile's `source` line fully
  updated (not just the `Cargo.toml` rev, leaving a stale lock), that no
  unrelated dependency shifted in `Cargo.lock` (a plain `cargo update -p
  tauri-nspanel` should only touch that crate's entry and its own
  transitive deps, not the whole tree), and that if a call-site fix was
  needed, it's the minimal change the new API requires — not a
  refactor of the surrounding window-setup code.
- Future maintenance: re-check this dependency's health again at the
  next deliberate native-window-behavior change (the same cadence
  `plans/README.md` already recommended for `smappservice-rs`), rather
  than letting it go stale for another 13 months by default.
