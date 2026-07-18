# Plan 026: Docs/DX truth pass — seven invoke commands everywhere, deadline-heartbeat prose, biome-gate wording, `just setup`

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a58f115..HEAD -- CLAUDE.md AGENTS.md docs/ARCHITECTURE.md docs/V5_TECHNICAL_SPEC.md docs/V3_6_TECHNICAL_SPEC.md justfile`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: docs / dx
- **Planned at**: commit `a58f115`, 2026-07-18

## Why this matters

Four agent-facing docs undercount the settings window's invoke-command
surface — and that count is exactly the tripwire the docs use to police
a security invariant. `CLAUDE.md`'s ipc section tells every agent:
never add a `#[tauri::command]` without listing it in `build.rs`,
otherwise it silently becomes callable from the overlay window and
breaks the receive-only guarantee. The code has **seven** gated
commands; the docs variously say "four" and "six". An agent trusting
the stale count can miscount the ACL surface it is supposed to protect.
Separately: the active v3.6 spec still says a deleted 250 ms polling
tick "decides rotation, full stop"; the command docs call `biome check`
"the gate" when the enforcing gate is `biome ci`; and `just test-all` —
the documented one-command local verification — fails on a fresh clone
because no recipe installs node dependencies. All small, all
truth-in-docs, all cheap.

## Current state

The single source of truth for the command surface —
`src-tauri/build.rs:8-16` (do not edit it; it is already correct):

```rust
        tauri_build::AppManifest::new().commands(&[
            "get_config",
            "get_default_config",
            "get_secret_status",
            "save_config_and_relaunch",
            "set_secret",
            "send_test_notification",
            "set_appearance",
        ]),
```

Seven commands. History for the prose: v5 shipped four (`get_config`,
`get_secret_status`, `save_config_and_relaunch`, `set_secret`); v5.1
(`efa1bd2`) added `send_test_notification` + `set_appearance` (six);
plan 020 (`9774930`, 2026-07-18) added `get_default_config` (seven).

The stale lines (verified at `a58f115`; ± a couple of lines if the
files drifted — grep, don't trust raw numbers):

1. `CLAUDE.md:23` — "…`set_appearance`, six invoke commands total…"
   (project-state narrative; `get_default_config` / plan 020 is absent
   from the whole file: `grep -c get_default_config CLAUDE.md` → 0).
2. `CLAUDE.md:178` — "the settings window's four invoke commands
   (`get_config`, `get_secret_status`, `save_config_and_relaunch`,
   `set_secret`)".
3. `AGENTS.md:23` — same "six invoke commands total" sentence as
   CLAUDE.md:23 (the two files mirror each other; keep them mirrored).
4. `AGENTS.md:176-178` — "the settings window … has six / invoke
   commands, gated per-window."
5. `docs/V5_TECHNICAL_SPEC.md:19` — "…a fourth tray item, six invoke
   commands, config validation…". Note the same file's `build.rs`
   snippet (~line 82) and command table (~line 123) already list all
   seven — the intro line is the only stale spot; the file is
   internally inconsistent.
6. `docs/ARCHITECTURE.md:507` — "a second, separately-scoped webview
   with exactly four invoke commands". This sits inside an "amended
   2026-07-17 (v5, §17)" block. ARCHITECTURE.md is the locked-decisions
   doc: **amend, never re-litigate** — add/extend an amendment note
   rather than silently rewriting history.
7. `docs/V3_6_TECHNICAL_SPEC.md:65` — "**fix**: the 250ms tick decides
   rotation, full stop; …" and `:103-108` — "…`tick()` runs
   unconditionally and periodically … harmless, just slower by up to
   250ms." Both predate plan 015 (`2cef7ad`), which replaced the fixed
   250 ms tick with deadline-based wakeups (`queue.rs::next_deadline`
   + a `tokio::sync::Notify`-woken `spawn_heartbeat` in `lib.rs`).
   The *later* parts of the same spec were already amended (line 440:
   "driving `tick()` (plan 015 — supersedes the original 250ms
   heartbeat…", lines 678, 794) — use their phrasing as the model, and
   leave them alone.
8. `CLAUDE.md:85` / `AGENTS.md:83` — "`npx biome check .` — frontend
   lint + format gate". The actual enforcing gate in
   `.github/workflows/ci.yml` and `justfile check-web` is
   `npx biome ci .` (read-only CI mode); `biome check` is the local dev
   command (`npm run lint:fix` applies fixes).
9. `justfile` — recipes call `npx …` / `cargo …` directly; there is no
   `setup`/`install` recipe, so `just test-all` fails on a fresh clone
   before `npm ci` has ever run. CI (`ci.yml` web job) runs `npm ci`
   first. The justfile's own header says it "mirrors
   .github/workflows/ci.yml exactly".

Conventions: all these docs are lowercase prose; CLAUDE.md and
AGENTS.md are near-mirrors — apply equivalent edits to both; counts
that live in `docs/TESTING_STRATEGY.md` §0 are out of scope here.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Count greps (after edits) | see Done criteria | as stated |
| justfile syntax | `just --list` (if `just` installed) or `just -f justfile --list` | lists recipes incl. `setup` |
| Nothing else builds/tests docs | — | — |

If `just` is not installed (`brew install just` per CLAUDE.md), verify
the justfile edit by eyeball + `git diff` only — do not install
software.

## Scope

**In scope** (the only files you should modify):
- `CLAUDE.md`
- `AGENTS.md`
- `docs/ARCHITECTURE.md` (the §17 amendment sentence only)
- `docs/V5_TECHNICAL_SPEC.md` (line ~19 only)
- `docs/V3_6_TECHNICAL_SPEC.md` (lines ~65 and ~103-108 only)
- `justfile`
- `plans/README.md` (status row)

**Out of scope** (do NOT touch, even though they look related):
- `src-tauri/build.rs`, `capabilities/*.json` — already correct.
- `docs/archive/**` — archived specs are historical records; stale by
  design.
- `docs/TESTING_STRATEGY.md` — its counts are maintained by test-adding
  plans, not this one.
- `docs/V3_6_TECHNICAL_SPEC.md` lines 440/678/794 — already amended by
  plan 015.
- `.github/workflows/ci.yml` — correct as-is.
- The uncommitted working-tree changes another session may have in
  `plans/022-unpark-deep-testing.md`, `prototype/status-rail.html`,
  `src/settings/preview-overlay.css` — never revert or stage them.

## Git workflow

- Branch: `advisor/026-docs-dx-truth-pass`.
- Commit style: `docs: seven invoke commands + deadline-heartbeat truth pass`
  and `dx: just setup recipe (npm ci)` (one or two commits, matching
  the repo's lowercase `area: summary` style).
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Fix the command counts (items 1–6)

- CLAUDE.md:23 + AGENTS.md:23: extend the narrative so the total reads
  seven and names the seventh, e.g. append after the v6 sentence: a
  short clause noting plan 020 (`9774930`, 2026-07-18) added
  `get_default_config` — seven invoke commands total. (Adjust the "six
  invoke commands total" phrase so no stale total survives; keeping the
  v5.1 sentence's own history is fine as long as the paragraph's final
  total is seven.)
- CLAUDE.md:178: "four invoke commands (…)" → "seven invoke commands
  (`get_config`, `get_default_config`, `get_secret_status`,
  `save_config_and_relaunch`, `set_secret`, `send_test_notification`,
  `set_appearance`)". Keep the surrounding build.rs warning text
  verbatim.
- AGENTS.md:176-178: "has six / invoke commands" → "has seven invoke
  commands" (mirror the CLAUDE.md list if the sentence structure
  allows; AGENTS.md's phrasing differs slightly — preserve its shape).
- docs/V5_TECHNICAL_SPEC.md:19: "six invoke commands" → "seven invoke
  commands".
- docs/ARCHITECTURE.md:507: extend the amendment, e.g. "…with exactly
  four invoke commands **(seven as of v5.1 + plan 020 — see
  `V5_TECHNICAL_SPEC.md` §2 for the current list)**…" or add a
  follow-on "amended 2026-07-18" sentence — the locked doc must show
  its history, not overwrite it.

**Verify**:
`grep -rn "six invoke\|four invoke" CLAUDE.md AGENTS.md docs/V5_TECHNICAL_SPEC.md` → no matches;
`grep -rn "four invoke" docs/ARCHITECTURE.md` → either no matches or only inside a sentence that also states seven;
`grep -c get_default_config CLAUDE.md AGENTS.md` → ≥1 each.

### Step 2: Fix the v3.6 rotation-driver prose (item 7)

- Line ~65: reword "the 250ms tick decides rotation, full stop" →
  rotation is decided in the rust core by `tick()`, driven since plan
  015 by deadline-based wakeups (`next_deadline` + `Notify`), not a
  fixed interval; the frontend still holds no duration logic (that
  half of the sentence is the point — keep it).
- Lines ~103-108: reword the "runs unconditionally and periodically …
  slower by up to 250ms" justification to the deadline model: the
  worst case is one heartbeat wake of added delay, and the fast path
  remains a latency optimization only. Model the phrasing on the
  already-amended line 440 block. Do not renumber sections or touch
  anything else in the file.

**Verify**: `sed -n '60,110p' docs/V3_6_TECHNICAL_SPEC.md` → no
remaining claim that a fixed 250 ms interval drives rotation;
`grep -n "250" docs/V3_6_TECHNICAL_SPEC.md` → remaining hits only in
the historical/amended contexts (lines ~440, ~678, ~794 or phrases
like "original 250ms").

### Step 3: Fix the biome-gate wording (item 8)

In CLAUDE.md:85 and AGENTS.md:83, replace the single line with a
version that distinguishes the two invocations, e.g.:

```
- `npx biome check .` — frontend lint + format, local dev command
  (`npm run lint:fix` auto-applies); the enforcing gate CI and
  `just check-web` run is `npx biome ci .`
```

**Verify**: `grep -n "biome" CLAUDE.md AGENTS.md` → both files mention
`biome ci` as the gate.

### Step 4: Add the `setup` recipe (item 9)

In `justfile`, after the `default` recipe, add:

```
# one-time / after-pull: install web deps (rust toolchain via rustup is
# a prerequisite, not installed here)
setup:
    npm ci
```

Then annotate `test-all`'s comment (or the file header) with one line:
fresh clones run `just setup` first — CI's web job does the equivalent
`npm ci`. Do NOT make `test-all` depend on `setup` (forcing a full
`npm ci` on every local run would make the loop slower than CI's cached
equivalent — keep it opt-in).

Also update the `just test-all` bullet in CLAUDE.md (~line 98) and its
AGENTS.md mirror: mention `just setup` once for fresh clones.

**Verify**: `grep -n "setup" justfile CLAUDE.md AGENTS.md` → recipe
exists and is documented in both docs. If `just` is installed:
`just --list` shows `setup`.

## Test plan

Docs-only plan — no code tests. The greps in each step and the Done
criteria are the verification. Run the frontend/rust suites only if you
touched something you shouldn't have (you'll know from `git status`).

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `grep -rn "six invoke\|four invoke" CLAUDE.md AGENTS.md docs/V5_TECHNICAL_SPEC.md` → no matches
- [ ] `grep -c get_default_config CLAUDE.md` ≥ 1 and same for AGENTS.md
- [ ] `docs/ARCHITECTURE.md` §17 amendment mentions the current count (seven) without deleting the historical "four" context
- [ ] `grep -n "250ms tick decides" docs/V3_6_TECHNICAL_SPEC.md` → no matches
- [ ] `grep -n "biome ci" CLAUDE.md AGENTS.md` → ≥1 match each
- [ ] `justfile` contains a `setup:` recipe running `npm ci`; `test-all` does NOT depend on it
- [ ] `git status` shows modifications only to in-scope files (the pre-existing uncommitted changes to `plans/022…`, `prototype/status-rail.html`, `src/settings/preview-overlay.css` and untracked `testing/` belong to another session — leave them exactly as found, unstaged)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- `src-tauri/build.rs` no longer lists exactly the seven commands
  quoted above — the docs fix would then be wrong; report the actual
  list instead of writing "seven".
- Any stale line's surrounding text differs materially from the
  excerpts (another docs pass may have landed).
- You are tempted to edit `docs/archive/**` or renumber spec sections.

## Maintenance notes

- The command count will go stale again the next time a
  `#[tauri::command]` is added — whichever plan adds one must update
  CLAUDE.md/AGENTS.md/V5 spec in the same change (plan 020 didn't,
  which is how this drift happened). Consider the count-adjacent
  wording "N invoke commands (see `build.rs` for the authoritative
  list)" to make future drift cheaper.
- CLAUDE.md and AGENTS.md are mirrors; a reviewer should diff the two
  files' edited regions against each other.
