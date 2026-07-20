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
> repo. **Also run** `sed -n '222,231p' src-tauri/src/lib.rs` and confirm
> it byte-matches the block quoted in "Current state" below — Step 2's
> fix pattern is written against that exact shape (line numbers and all),
> and `lib.rs` is one of the most actively-touched files in this repo, so
> unlike the dependency pin it can drift on its own schedule. On a
> mismatch, treat it as a STOP condition per the template's standard
> drift-check rule, not something to improvise around.

## Status

- **Priority**: P2
- **Effort**: S–M
- **Risk**: MED — confirmed (not just suspected) compile break at the
  one call site this app uses: `to_panel()` and `set_style_mask()` both
  changed signature between the pinned rev and the target rev (see
  "Confirmed breaking changes" below). The fix pattern is known and
  small, but it requires adopting the crate's `tauri_panel!` macro (new
  surface this app doesn't use today) and a manual runtime-equivalence
  check, not just a lockfile change.
- **Depends on**: none
- **Category**: dependency
- **Planned at**: commit `f2cbae6`, 2026-07-19 — dependency facts below
  verified live against `github.com/ahkohd/tauri-nspanel` on that date;
  **re-verify the target rev is still current before executing**, since
  upstream may have moved further by execution time.
- **Review-plan pass (2026-07-20)**: independently re-verified this
  plan's dependency claims against a live fetch of the upstream repo
  (`git ls-remote`, `gh api .../compare/...`, and direct file reads of
  both revs' `src/lib.rs`) rather than trusting the prior research.
  Two corrections, both folded into "Current state"/Step 0/Step 2 below:
  (1) **"the ONLY call into `tauri_nspanel`" was wrong** — `lib.rs:178`
  (`.plugin(tauri_nspanel::init())`) is a second, real call site the
  plan never mentioned or accounted for; confirmed its signature is
  unchanged between the two revs, so it's not a risk, but the plan's own
  claim didn't match what its own suggested `grep -rn` would show.
  (2) **the "may hit a compile break" framing was too hedged** — a
  direct signature diff of both revs confirms `to_panel()` and
  `set_style_mask()` (the two symbols this app actually calls) BOTH
  changed in ways this app's exact call shape will not compile against;
  this is not a maybe, and the concrete before/after + a working fix
  pattern from the upstream repo's own examples are now inlined below
  so the executor doesn't have to rediscover them from scratch. The
  target rev (`a3122e894383aa068ec5365a42994e3ac94ba1b6`) and the
  39-commits-ahead count were both re-confirmed exactly correct — no
  drift there. Also checked and confirmed a non-issue: the new crate
  requires `tauri = "2.8.5"` with the `macos-private-api` feature; this
  app already pins `tauri = "2"` (resolved `2.11.5` in `Cargo.lock`,
  satisfies `^2.8.5`) with `macos-private-api` already enabled
  (`Cargo.toml:16,41`) — no knock-on tauri-version or feature-flag
  change needed.
- **Review-plan pass (2026-07-19, third pass)**: independently
  re-verified this plan live, from scratch, rather than trusting either
  prior pass's summary. `git ls-remote` against the upstream repo
  confirms `v2` is still exactly `18ffb9a...` (matches this repo's pin —
  zero drift) and `v2.1` is still exactly
  `a3122e894383aa068ec5365a42994e3ac94ba1b6` (matches this plan's
  target — unchanged since the prior pass), and no branch or tag newer
  than `v2.1` exists upstream. Repo health re-confirmed via `gh api
  repos/ahkohd/tauri-nspanel`: not archived, `pushed_at` as recently as
  2026-07-10 (some branch other than `v2.1` itself, which hasn't moved,
  but proof the repo isn't dormant), 409 stars, 11 open issues — no
  abandonment signal, the STOP condition on upstream health does not
  trigger. Both breaking-change diffs (`to_panel`/`set_style_mask`/
  `init`) were re-read directly from the crate's raw `src/lib.rs` and
  `src/raw_nspanel.rs` at both revs via `gh api .../contents/...?ref=
  <sha>` — byte-for-byte match to what this plan already states; no
  correction needed there. Two new findings, folded into "Confirmed
  breaking changes" and Step 2 below: (1) the
  `NSWindowStyleMask::NonactivatingPanel` replacement value is now
  **proven**, not just doc-referenced — this machine's own local cargo
  registry cache has `objc2-app-kit` 0.3.2's real generated source
  (`objc2-app-kit-0.3.2/src/generated/NSWindow.rs:54`: `const
  NonactivatingPanel = 1<<7;`), confirming bit-for-bit equivalence with
  today's `1 << 7` — the "confirm at execution time" hedge on that
  specific swap can be dropped. (2) the plan's "existing intent... never
  key/main window" framing is **not actually true of today's
  behavior** — see the correction below.

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

  **Correction (review-plan pass, 2026-07-20): this is NOT the only call
  into `tauri_nspanel`.** `grep -rn "tauri_nspanel\|nspanel" src-tauri/src`
  actually returns a second real hit: `lib.rs:178`,
  `.plugin(tauri_nspanel::init())`, the plugin registration in the
  `tauri::Builder` chain (confirmed by direct read — it is NOT inside
  the `#[cfg(target_os = "macos")]` block quoted above, it's earlier in
  the same `run()` function). This app is macOS-only by build target
  (per `CLAUDE.md`: rust CI runs on `macos-latest` only, and
  `tauri-nspanel` is a macOS-only Cargo dependency), so `init()` being
  unconditional is fine and not itself a bug — but the plan's own claim
  that lines 222-231 are "the ONLY call" doesn't match what its own
  suggested grep shows, and `init()` was never checked for a signature
  change. It has now been checked (see "Confirmed breaking changes"
  below): `init()`'s signature (`pub fn init<R: Runtime>() ->
  TauriPlugin<R>`) is byte-identical between the pinned rev and the
  target rev — confirmed safe, but for a reason the original plan never
  established.

- Proposed target: `github.com/ahkohd/tauri-nspanel`'s `v2.1` branch tip,
  `a3122e894383aa068ec5365a42994e3ac94ba1b6` — re-confirmed exactly
  correct via `git ls-remote https://github.com/ahkohd/tauri-nspanel
  refs/heads/v2.1` on 2026-07-20 (review-plan pass); **still re-verify
  this is the branch tip at execution time** (Step 1), since it may move
  further before the plan is actually run.

### Confirmed breaking changes (review-plan pass, 2026-07-20 — verified via `gh api repos/ahkohd/tauri-nspanel/contents/src/lib.rs?ref=<rev>` at both revs)

Both symbols this app calls in the 222-231 block changed shape. This is
not speculative — it's a direct diff of the crate's own `src/lib.rs` at
the pinned rev vs. the target rev:

- **`to_panel()` gained a required generic type parameter and changed
  its return type.** Pinned rev: `fn to_panel(&self) -> tauri::Result<
  ShareId<RawNSPanel>>` (no type parameter). Target rev: `fn
  to_panel<P: FromWindow<R> + 'static>(&self) -> tauri::Result<
  PanelHandle<R>>` — `P` cannot be inferred from context (it doesn't
  appear in the return type, which is `PanelHandle<R> = Arc<dyn
  Panel<R>>` regardless of `P`), so a bare `window.to_panel()` (this
  app's exact current call, `lib.rs:226`) will not compile. Every
  current usage in the upstream repo's own examples defines a minimal
  panel type via the crate's `tauri_panel!` macro first, then passes it
  explicitly: `examples/basic/src-tauri/src/main.rs` (target rev) does
  ```rust
  use tauri_nspanel::{tauri_panel, WebviewWindowExt};

  tauri_panel! {
      panel!(Panel {
          config: {
              can_become_key_window: true,
              can_become_main_window: false
          }
      })
  }
  // ...
  let panel = window.to_panel::<Panel>().unwrap();
  ```
  **Correction (review-plan pass, 2026-07-19, third pass): this app's
  "existing intent" is NOT "never key/main window" — that framing was
  never actually verified against the pinned crate's real behavior, and
  it's wrong.** Direct read of the pinned rev's
  `src/raw_nspanel.rs:47-49` shows `RawNSPanel`'s `canBecomeKeyWindow`
  override is hardcoded unconditionally: `extern "C" fn
  can_become_key_window(_: &Object, _: Sel) -> BOOL { YES }` — every
  panel this crate creates CAN become a key window, full stop, with no
  per-instance opt-out exposed at this pinned rev. `grep -n
  "becomes_key\|set_style_mask\|to_panel\|nspanel" src-tauri/src/lib.rs`
  confirms this app never calls the crate's
  `set_becomes_key_only_if_needed` (the one API that could have
  suppressed this) — so today's actual, verified behavior is "the panel
  CAN become key," not "never." `can_become_main_window: false` is a
  separate, uncontested case: `RawNSPanel` doesn't override
  `canBecomeMainWindow` at all in the pinned rev, so it inherits
  `NSPanel`'s own AppKit default of `NO` — `false` here is a correct
  behavior-preserving choice. **For `can_become_key_window`, set it to
  `true`** to match today's actual (if likely accidental) behavior — do
  NOT set it to `false` on the assumption that "matches nonactivating
  intent," since that would be a real behavior change (a panel that
  currently CAN become key would stop being able to), not a bump-only
  change. If the operator wants the panel to genuinely never become key
  (arguably the more correct behavior for a nonactivating overlay that
  should never take keyboard focus), that's a legitimate improvement —
  but it's a scope decision for a different plan, not something to slip
  in silently under a dependency-bump plan's "matches existing intent"
  banner. The exact remaining `config:` fields (beyond these two
  resolved ones) are a judgment call for whoever executes this plan; the
  upstream examples directory (`examples/basic/`,
  `examples/hover_activate/`, `examples/panel_style_mask.rs`, all at the
  target rev) is the reference to work from, not a blank-page
  reinvention.
- **`set_style_mask()` changed its parameter type from a raw integer to
  a typed bitflags value.** Pinned rev (on `RawNSPanel`, in
  `src/raw_nspanel.rs`): `pub fn set_style_mask(&self, style_mask:
  i32)`. Target rev (on the `Panel<R>` trait, in `src/lib.rs`): `fn
  set_style_mask(&self, style_mask: objc2_app_kit::NSWindowStyleMask)`
  — this app's exact current call, `panel.set_style_mask(1 << 7)`
  (`lib.rs:230`), passes a bare integer and will not compile. The
  needed replacement constant is confirmed to exist:
  `objc2_app_kit::NSWindowStyleMask::NonactivatingPanel` (verified
  against `objc2-app-kit` 0.3.2's own docs — this app already pins
  exactly that version, `Cargo.toml:36`, with the `NSWindow` feature
  enabled, and already imports `objc2_app_kit` types the same way in
  `apply_overlay_native_config`, e.g.
  `NSWindowCollectionBehavior::CanJoinAllSpaces` at `lib.rs:528` — same
  crate, same PascalCase bitflags-constant convention). **Confirmed, not
  just likely (review-plan pass, 2026-07-19, third pass): the
  replacement is `panel.set_style_mask(objc2_app_kit::NSWindowStyleMask
  ::NonactivatingPanel);`.** This machine's own local cargo registry
  cache has `objc2-app-kit` 0.3.2's real generated source
  (`~/.cargo/registry/src/index.crates.io-*/objc2-app-kit-0.3.2/src/
  generated/NSWindow.rs:54`): `const NonactivatingPanel = 1<<7;` — the
  bitflags constant's raw value is bit-for-bit identical to today's
  `1 << 7`, proven from the crate's own generated bindings rather than
  inferred from docs. The `set_style_mask` swap itself is a verified
  no-op on runtime behavior; the only real behavior-preservation
  question in this fix is the `can_become_key_window` config field
  addressed above, which `set_style_mask` alone never touched either
  before or after this bump.
- **Net effect on this plan's Step 2**: `cargo build --locked` failing
  at these two symbols is the *expected* outcome of Step 2, not a
  contingency — treat the "if it fails, fix it" framing there as "when
  it fails, fix it using the pattern above," while still honoring the
  STOP condition for any failure *outside* these two symbols.

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
   (`18ffb9a201fbf6fedfaa382fd4b92315ea30ab1a`) and your target tip (if
   your target tip is still `a3122e894383aa068ec5365a42994e3ac94ba1b6`,
   this has already been done for you — see "Confirmed breaking
   changes" in Current state above; skip straight to confirming your
   target tip hasn't moved, then proceed to Step 1). If your target tip
   HAS moved past `a3122e894383aa068ec5365a42994e3ac94ba1b6`, re-do this
   check for the additional commits: specifically look for any further
   change to `to_panel()`/`set_style_mask()`/`WebviewWindowExt`/`init()`
   beyond what's already documented above (those four symbols are the
   ones this app calls). The commits titled `rename method` and `feat:
   support snake_case config in panel! macro` (both present in the
   already-reviewed range) were checked and confirmed NOT to affect this
   app: `rename method` only renames an internal macro-local helper
   function private to the crate's own `src/panel.rs`, and the
   `panel!`/`snake_case config` work is part of the `tauri_panel!` macro
   path this app doesn't use today (though the fix below adopts it).

**Verify**: you can state the exact target rev and confirm (or, if the
tip moved, re-derive) which of this app's four call-site symbols
(`init`, `WebviewWindowExt`, `to_panel`, `set_style_mask`) changed. If
you cannot reach the network to do this, STOP (see STOP conditions) —
do not guess at a rev.

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

**Expect `cargo build --locked` to fail here** — this is the confirmed,
not hypothetical, outcome (see "Confirmed breaking changes" in Current
state above): `to_panel()` needs an explicit type parameter now, and
`set_style_mask()` needs a typed `objc2_app_kit::NSWindowStyleMask`
argument instead of a raw integer. Fix the call site using the pattern
already laid out above (a `tauri_panel!`-defined minimal panel type,
`window.to_panel::<ThatType>()`, and
`panel.set_style_mask(objc2_app_kit::NSWindowStyleMask::NonactivatingPanel)`
in place of `1 << 7`) — this is the one case where touching `lib.rs`
beyond the 222-231 block is in scope (the `tauri_panel!` macro
invocation needs to live somewhere in the file, e.g. near the top
alongside other `use`/type declarations), but keep the fix as small as
the actual API change requires: don't adopt anything from the new
crate's richer surface (event handlers, corner radius, transparency,
`PanelBuilder`) beyond what's needed to compile and preserve today's
exact behavior — borderless, nonactivating style mask, `can_become_key_
window: true` (matches the pinned rev's hardcoded `YES`, see the
correction above — do NOT set this to `false`), `can_become_main_
window: false`. If the failure is anywhere else — not one of the two
named symbols, or a transitive dependency conflict, or an unrelated
crate — STOP; that's a sign the bump has a wider blast radius than this
plan (and this review-plan pass's confirmed diff) scoped for.

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
version bump with no *intended* behavior change to notchtap's own code,
even though Step 2's call-site fix (confirmed needed — see "Confirmed
breaking changes") touches real code. The existing test suite, which
exercises `apply_overlay_native_config` and window setup only
indirectly via integration-style tests, is the safety net; do not
invent a new unit test for a third-party crate's internals — the manual
smoke check in Step 4 is what actually verifies the panel still behaves
correctly, since none of the existing automated tests can observe
`NSPanel` conversion behavior on a real window.

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
- `cargo build --locked` fails anywhere other than the two named
  call-site symbols in `lib.rs:222-231` (`to_panel`/`set_style_mask` —
  see "Confirmed breaking changes" in Current state) — a wider break
  means the bump's blast radius is larger than this plan scoped for.
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
  transitive deps, not the whole tree), and that the call-site fix
  (confirmed needed — see "Confirmed breaking changes" in Current
  state) is the minimal change the new API requires: a `tauri_panel!`
  macro block sized to this app's actual needs, not an adoption of the
  new crate's richer surface (event handlers, `PanelBuilder`, corner
  radius, transparency) this app has no use for.
- **A reviewer should specifically check `can_become_key_window` is set
  to `true`, not `false`, in the executor's `panel!` macro block** (see
  the review-plan-pass correction in "Confirmed breaking changes" —
  `RawNSPanel::canBecomeKeyWindow` is hardcoded `YES` at the pinned rev,
  so `true` is the behavior-preserving choice; `false` would be an
  unreviewed behavior change riding along on a dependency bump). If the
  executor chose `false`, that's not necessarily wrong, but it must be
  called out explicitly in their report as an intentional behavior
  change, not treated as bump-only.
- Future maintenance: re-check this dependency's health again at the
  next deliberate native-window-behavior change (the same cadence
  `plans/README.md` already recommended for `smappservice-rs`), rather
  than letting it go stale for another 13 months by default.
