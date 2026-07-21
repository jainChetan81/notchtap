# Plan 075: SPIKE — trial-bump the frontend toolchain (TypeScript 7 / Vite 8 / Vitest 4) in an isolated worktree

> **Executor instructions**: This is a SPIKE plan, matching the shape of
> plans 030/031/049-053 — the deliverable is a trial result and a written
> recommendation, not a merged dependency bump. Do NOT change
> `package.json` on the main working tree; do all trial work in an
> isolated worktree/branch and report findings. When done, update the
> status row for this plan in `plans/README.md` — unless a reviewer
> dispatched you and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- package.json`
> If the file changed since this plan was written, re-check current pinned
> versions against the "Current state" section before proceeding.

## Status

- **Priority**: P3
- **Effort**: S (spike)
- **Risk**: MED (the trial itself is isolated/reversible; the risk is in
  a *future* decision to actually adopt the bump, which this plan does
  not make)
- **Depends on**: none
- **Category**: migration
- **Planned at**: commit `f6c2f46`, 2026-07-20

## Why this matters

`package.json` pins `typescript: "~5.8.3"`, `vite: "^7.0.4"` (resolved
`7.3.6`), `vitest: "^3.2.0"` (resolved `3.2.7`), and
`@vitejs/plugin-react: "^4.6.0"` (resolved `4.7.0`). As of this plan's
writing, live registry checks show meaningfully newer majors available:
TypeScript `7.0.2` (a full Go-native compiler rewrite — "Project Corsa,"
GA'd 2026-07-08, ~10x faster, but early coverage flags some framework
plugin ecosystems as not yet fully caught up), Vite `8.1.5` (GA'd
2026-03-12, switched to the Rolldown/Rust bundler), Vitest `4.1.10` (same
release wave, tracking Vite 8), and `@vitejs/plugin-react` `6.0.3`.

This repo's last full dependency audit (2026-07-17, recorded in
`plans/README.md`'s rejected-findings list) judged "all deps
current-generation for 2026" — reasonable at the time, but three of the
frontend toolchain's core pieces have since undergone coordinated major
rewrites in the ~3 months since that verdict. This isn't an EOL or
security-cutoff problem — 5.8.3/7.x/3.x all still function fine — but the
gap compounds the longer it's deferred, and jumping onto a brand-new
Go-native typechecker plus a brand-new Rust bundler simultaneously, later
and all at once, is a bigger single move than trialing it now while the
repo is still a manageable size.

This plan does NOT decide whether to adopt the bump — TS7 was 12 days old
at audit time and framework-ecosystem compatibility (React 19,
`@vitejs/plugin-react`, Biome 2.5.4, `jsdom`) against this exact
combination hasn't been hands-on verified. The plan's job is to produce
that verification, in an isolated, disposable worktree, and report a
clear go/no-go recommendation with evidence — the same shape as this
repo's existing design-spike plans (030/031/049-053), just for a
dependency trial instead of a design doc.

## Current state

- `package.json` (repo root), relevant lines (re-confirm exact line
  numbers and resolved versions yourself — package managers move fast,
  don't trust the numbers below without a fresh check):

  ```json
  "dependencies": {
    "react": "^19.1.0",
    ...
  },
  "devDependencies": {
    "@vitejs/plugin-react": "^4.6.0",
    ...
    "typescript": "~5.8.3",
    "vite": "^7.0.4",
    "vitest": "^3.2.0"
  }
  ```

- `biome.json` (repo root) — Biome 2.5.4's config; confirm it doesn't
  have any TypeScript-version-specific settings that a TS7 bump could
  interact with.
- `vite.config.ts` — the current Vite 7 config (fixed dev port 1420,
  `strictPort`, HMR via `TAURI_DEV_HOST`) — this is standard Tauri
  boilerplate; check it against Vite 8's migration guide for any breaking
  config-shape changes.
- `tsconfig.json`/`tsconfig.node.json` — current strict settings
  (`strict`, `noUnusedLocals`, `noUnusedParameters`,
  `noFallthroughCasesInSwitch`) — TS7's stricter/rewritten type-checking
  could plausibly surface new errors under these settings that TS5
  didn't catch; that's exactly what this spike needs to find out.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Create isolated worktree | `git worktree add ../notchtap-ts7-spike -b spike/ts7-vite8-trial` | new worktree created |
| Trial-bump versions | (in the worktree) edit `package.json`'s `typescript`/`vite`/`vitest`/`@vitejs/plugin-react` to their latest majors, then `npm install` | lockfile updates, no install errors |
| Typecheck | `npx tsc --noEmit` | records pass/fail + error count if any |
| Lint | `npx biome ci .` | records pass/fail |
| Tests | `npx vitest run` | records pass/fail + count vs. the pre-bump baseline you record in Step 3 (183 as of 2026-07-21 per `docs/TESTING_STRATEGY.md` §0 — but Step 3's own run is the authoritative baseline, never a number from this plan) |
| Build | `npx vite build` | records pass/fail |
| Dev server smoke | `npm run tauri dev` (if you can drive a GUI) | app launches without a blank/broken webview |
| Clean up | `git worktree remove ../notchtap-ts7-spike` (after recording findings — do NOT merge or push this branch) | worktree removed |

## Scope

**In scope**:
- A disposable git worktree/branch for the trial (never merged as part of
  this plan)
- A written trial report (becomes this plan's completion note / a short
  section appended to this plan file, or a new `docs/design/` doc if the
  findings are substantial enough to warrant one — your call based on
  how much there is to say)

**Out of scope**:
- Any change to the main working tree's `package.json`/`package-lock.json`
  — this plan is exploratory only. If the trial fully succeeds and you
  believe adoption is safe, that becomes a NEW plan recommendation (a
  follow-up plan number), not something this plan executes directly.
- Any source code change to work around a trial failure (e.g. rewriting
  code to satisfy TS7's stricter checking) — if the trial finds errors,
  record them; don't fix them as part of a "spike."

## Steps

### Step 1: Set up the isolated worktree

From the repo root (`/Users/chetanjain/Desktop/code/mac-notification-nudge`):

```
git worktree add ../notchtap-ts7-spike -b spike/ts7-vite8-trial
cd ../notchtap-ts7-spike
```

**Verify**: `git worktree list` shows the new worktree; `pwd` confirms you're in it before proceeding.

### Step 2: Check live latest versions (don't trust this plan's numbers)

```
npm view typescript version
npm view vite version
npm view vitest version
npm view @vitejs/plugin-react version
```

Record what you get — more time may have passed since this plan was
written, and even newer versions may exist now.

**Verify**: 4 version strings recorded.

### Step 3: Establish the pre-bump baseline (still at pinned versions)

Before touching `package.json`, install and run all four gates at the
CURRENT pinned versions in the worktree:

```
npm ci
npx tsc --noEmit
npx biome ci .
npx vitest run
npx vite build
```

Record each result (exit code + test count). This is the baseline every
post-bump result is compared against — without it, a post-bump failure
can't be attributed to the bump vs. a pre-existing environment problem.

**Verify**: all four gate results + the vitest count recorded. If ANY
gate fails here, at the pinned versions, STOP and report — the trial
cannot produce attributable evidence on a broken baseline.

### Step 4: Bump and install

Edit `package.json` in the worktree to the latest majors found in Step 2
(exact syntax: replace `~5.8.3`→`^<latest TS>`, `^7.0.4`→`^<latest
Vite>`, `^3.2.0`→`^<latest Vitest>`, `^4.6.0`→`^<latest plugin-react>`).
Run `npm install`.

**Verify**: `npm install` exits 0, no peer-dependency conflict errors printed (record them if any appear even on success — `npm` sometimes warns rather than failing).

### Step 5: Run every gate again, record pass/fail + details vs. baseline

```
npx tsc --noEmit
npx biome ci .
npx vitest run
npx vite build
```

For each: record exit code, and if non-zero, the actual error output
(trimmed to the relevant lines, not the full noise), compared explicitly
against Step 3's baseline result for the same gate — this delta is the
concrete evidence the go/no-go recommendation rests on. A vitest count
differing from Step 3's baseline count is a finding even if the run
passes.

**Verify**: each command's result recorded as a pass/fail + delta-vs-baseline pair.

### Step 6: Dev-server smoke (best-effort)

If you can drive a GUI (or the environment supports headless Tauri dev
launch), run `npm run tauri dev` and confirm the app launches without a
blank/broken webview. If you can't, say so explicitly rather than
claiming this step passed.

**Verify**: recorded pass/fail/not-attempted.

### Step 7: Write the recommendation, clean up

Summarize: did every gate pass cleanly? If not, what broke and how
severe does the fix look (a config tweak vs. real source changes)? Give
a clear go/no-go-for-now recommendation. Then:

```
cd /Users/chetanjain/Desktop/code/mac-notification-nudge
git worktree remove --force ../notchtap-ts7-spike
```

(`--force` is required and expected here: the worktree is deliberately
dirty — the bumped `package.json`/`package-lock.json` were never meant
to be committed. This is the one sanctioned `--force` in this plan; it
discards only the disposable trial state.)

Do NOT push or merge `spike/ts7-vite8-trial` — delete the branch too
once findings are recorded (`git branch -D spike/ts7-vite8-trial`), since
nothing about this trial should persist as a mergeable branch.

**Verify**: `git worktree list` no longer shows the spike worktree; on the main tree, `git diff --stat -- package.json package-lock.json` is empty (the main tree's toolchain pins are untouched — other files, like this plan's completion note and `plans/README.md`, are expected to change).

## Test plan

No new automated tests — this is a one-time exploratory trial in a
disposable worktree. The four gate commands, run at pinned versions
(Step 3) and again post-bump (Step 5), ARE the verification; there's
nothing to add to the permanent test suite.

## Done criteria

- [ ] Worktree created; all 4 gates (tsc/biome/vitest/vite build)
      executed TWICE — at pinned versions (baseline) and post-bump —
      with both result sets and the delta recorded
- [ ] Dev-server smoke attempted or explicitly flagged not-attempted
- [ ] A clear written go/no-go recommendation exists (in this plan's
      completion note or a new `docs/design/` doc if warranted)
- [ ] The trial worktree and branch are both cleaned up — nothing merged, nothing pushed
- [ ] The main working tree is unchanged (`git status`/`git diff --stat` empty on `package.json`)
- [ ] `plans/README.md` status row for 075 updated with the recommendation summary

## STOP conditions

- Any gate fails in Step 3, at the PINNED versions — the environment
  itself is broken and no post-bump result can be attributed to the
  bump; report the baseline failure instead of proceeding.
- `npm install` fails outright with an unresolvable peer-dependency
  conflict — record the exact error and stop; that itself is a strong
  "not yet" signal worth reporting rather than fighting through with
  `--force`/`--legacy-peer-deps` (which would mask exactly the
  compatibility signal this spike exists to surface).
- Any step would require modifying the main working tree rather than the
  isolated worktree — refuse and re-check you're in the right directory.

## Maintenance notes

- If the recommendation is "adopt," that becomes a new plan (next
  available number after this one) that actually bumps `package.json` on
  a real branch, not a re-run of this spike.
- If the recommendation is "not yet," note what specifically blocked it
  (a named incompatibility) so a future re-trial knows what to check
  first rather than repeating this plan's full sweep from scratch.
- This is the kind of gap this repo has now flagged and closed cleanly
  once (`smappservice-rs`'s "needs network" item, closed during this same
  audit session after being carried for 3 prior sessions) — don't let
  this one linger un-revisited for that long; a stale "we should check
  this" is worse than either a clear yes or a clear not-yet.

**Review-plan pass (2026-07-21)**: Verified at HEAD `647f6d0`. All
version claims re-checked and confirmed live: `package.json` still pins
`typescript ~5.8.3` / `vite ^7.0.4` / `vitest ^3.2.0` /
`@vitejs/plugin-react ^4.6.0` / `react ^19.1.0`; lockfile resolves
5.8.3 / 7.3.6 / 3.2.7 / 4.7.0 exactly as stated;
`git diff --stat f6c2f46..HEAD -- package.json` is empty (no drift);
registry latest per `npm view` today is still exactly TS 7.0.2 /
Vite 8.1.5 / Vitest 4.1.10 / plugin-react 6.0.3; Biome is 2.5.4;
`tsconfig.json` has the four named strict flags; `vite.config.ts` has
port 1420 / `strictPort` / `TAURI_DEV_HOST`. One fix applied: the vitest
baseline count was stale (112 → 181; plans 080-086 landed 2026-07-20/21
after this plan's planned-at SHA). Plan is ready to execute as written.

**Review-plan pass 2 (2026-07-21, at `62d6eb0`)**: re-verified after
plan 087 (hover primitive) merged — `package.json`/`package-lock.json`
still have zero drift from `f6c2f46` (the drift check stays valid as
written) and registry latests are unchanged (TS 7.0.2 / Vite 8.1.5 /
Vitest 4.1.10 / plugin-react 6.0.3, re-run this pass). The vitest count
moved again (181 → 183 with 087), which exposed a structural weakness:
the plan pinned a baseline count that goes stale with every landing, and
bumped versions immediately after worktree creation, so a post-bump gate
failure couldn't be attributed (bump vs. pre-broken environment). Fixed
by inserting Step 3 (run all four gates at pinned versions in the
worktree first; its live run — not any number in this plan — is the
baseline), renumbering the old Steps 3-6 to 4-7, requiring Step 5 to
record explicit delta-vs-baseline pairs, and adding a matching STOP
condition (baseline failure = report, don't proceed). Done criteria
updated to require both result sets.

## Completion note (executed 2026-07-21, base commit `f084fa8`)

Executed in an already-isolated agent worktree (no nested worktree
created — the dispatching agent owns worktree lifecycle; this satisfies
the plan's real intent since the main tree was never reachable).

**Step 2 — live versions** (all matched the plan's pre-verified numbers
exactly, no drift): TypeScript `7.0.2`, Vite `8.1.5`, Vitest `4.1.10`,
`@vitejs/plugin-react` `6.0.3`.

**Baseline vs. post-bump — all four gates:**

| Gate | Baseline (pinned: TS 5.8.3 / Vite 7.3.6 / Vitest 3.2.7 / plugin-react 4.7.0) | Post-bump (TS 7.0.2 / Vite 8.1.5 / Vitest 4.1.10 / plugin-react 6.0.3) | Delta |
|---|---|---|---|
| `npx tsc --noEmit` | exit 0, no output | exit 0, no output | none — zero new type errors under TS7 + existing strict flags |
| `npx biome ci .` | exit 0, "Checked 35 files in 38ms. No fixes applied." | exit 0, "Checked 35 files in 61ms. No fixes applied." | none — no config-discovery quirk hit either side |
| `npx vitest run` | exit 0, **13 test files / 183 tests passed** | exit 0, **13 test files / 183 tests passed** | none — identical count, matches `docs/TESTING_STRATEGY.md` §0's 183 exactly |
| `npx vite build` | exit 0, 2222 modules transformed, built in 1.01s | exit 0, 2210 modules transformed, built in 0.467s | none functionally; build ~2x faster and slightly fewer modules/smaller bundles (Rolldown), no config-shape change needed — `vite.config.ts` (port 1420, `strictPort`, `TAURI_DEV_HOST`) worked unmodified against Vite 8 |

`npm install` for the bump exited 0 with no peer-dependency conflicts or
warnings (9 packages added, 51 removed, 17 changed).

**Step 6 — dev-server smoke: NOT ATTEMPTED.** This execution has no
GUI-driving capability available; `npm run tauri dev` opens a native
webview window that can't be observed or verified without one. Not
claimed as passed.

**Recommendation: GO** (with one caveat). Every automated gate —
typecheck, lint, unit tests, build — passed identically at the bumped
majors with zero source or config changes required. `vite.config.ts`
needed no migration-guide adjustments; the strict `tsconfig.json` flags
surfaced no new errors under TS7's rewritten checker;
`@vitejs/plugin-react` 6.x plugged into Vite 8 without incident; Biome
2.5.4 has no TS-version-coupled settings that reacted. The one
open item before full adoption is Step 6 — the dev-server/webview smoke
test — which this execution could not perform; a future adoption pass
should run `npm run tauri dev` by hand on the target mac before merging,
since none of the four automated gates exercise the actual Tauri
webview boot path. No other blocker was found.

**For a future adoption plan**: bump `package.json` exactly as trialled
here (`typescript` `~5.8.3`→`^7.0.2`, `vite` `^7.0.4`→`^8.1.5`, `vitest`
`^3.2.0`→`^4.1.10`, `@vitejs/plugin-react` `^4.6.0`→`^6.0.3`), run
`npm install`, then do the one thing this spike couldn't: an actual
`npm run tauri dev` visual check on the mac mini (HUD mode) before
calling it done.

**Cleanup confirmation**: `package.json`/`package-lock.json` reverted
via `git checkout`; `git diff --stat -- package.json package-lock.json`
and `git status --porcelain` both empty; `node_modules` reinstalled via
`npm ci` to match the reverted lockfile. No nested worktree or branch
was created (per dispatch-time deviation from Step 1/Step 7), so there
is nothing to `git worktree remove` or `git branch -D`.

## Reviewer verification (2026-07-21, base `f084fa8`)

A four-gates-all-green spike result is exactly the shape that warrants
independent re-running rather than acceptance, so the reviewer re-ran
the load-bearing gates directly in the executor's worktree:

- **`tsc --noEmit` under TypeScript 7.0.2** — re-installed TS 7.0.2 and
  re-ran: exit 0, no output. Confirmed. This was the single most
  surprising claim (a Go-native compiler rewrite producing zero new
  errors against `strict` + `noUnusedLocals` + `noUnusedParameters` +
  `noFallthroughCasesInSwitch`) and it holds.
- **`vite build` under Vite 8.1.5 / plugin-react 6.0.3** — exit 0, built
  in 342ms. Confirmed.
- **`vitest run` under Vitest 4.1.10** — 13 files / **183 passed**,
  matching `docs/TESTING_STRATEGY.md` §0 exactly. Confirmed.
- **Scope** — diff is one file, +54/-0 (pure append).
  `git diff --stat -- package.json package-lock.json` empty in both the
  worktree and the main tree. No source, config, or tsconfig file
  touched.

**One invariant the executor did not check, verified here and passing**:
plan 082 vendored 12 Meteocons SVGs that must be emitted as real asset
files, not inlined as data URIs — an invariant held today only by the
`?no-inline` query suffix in `src/lib/weatherArt.ts`. Since Vite 8
swaps the bundler wholesale (Rollup → Rolldown), asset-inlining
behavior is a plausible silent regression that **no gate would catch**
(tsc, biome, vitest and the build's own exit code are all blind to it).
Checked explicitly post-bump: `dist/assets/*.svg` = 12 files, zero
`data:image/svg` occurrences in emitted JS. The escape hatch survives
Rolldown.

**Verdict: APPROVE the spike, and concur with GO — with the caveat
sharpened.** The recommendation rests on genuinely re-verified evidence,
not self-report. Two items the future adoption plan must own, neither of
which is a blocker:

1. **Step 6 (Tauri webview boot) remains unverified** and is correctly
   flagged not-attempted rather than fake-passed. This matters more than
   the executor's framing implies: the four automated gates verify the
   *build*, and none of them boot the app. Vite 8's dev server is the
   piece Tauri actually consumes in `npm run tauri dev`, so the dev-path
   config surface (port 1420, `strictPort`, `TAURI_DEV_HOST` HMR) is
   precisely what stays untested — a passing `vite build` is not
   evidence for it. Hands-on check on the mac mini before merging.
2. **The module-count delta (2222 → 2210) is unexplained.** Almost
   certainly Rolldown's differing module accounting rather than 12
   dropped modules, and the SVG check above rules out the most alarming
   candidate — but it is an assumption, not a measurement. The adoption
   plan should diff the emitted `dist/` asset list pre- and post-bump
   and confirm the sets match.
