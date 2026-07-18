# Plan 029: Pin GitHub Actions to commit SHAs (match the repo's Cargo-side supply-chain posture)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a58f115..HEAD -- .github/workflows/ci.yml`
> If the workflow changed since this plan was written, re-inventory the
> `uses:` lines before proceeding.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW (additive hardening; only cost is periodic pin bumps)
- **Depends on**: none (requires network access to resolve tag→SHA)
- **Category**: security
- **Planned at**: commit `a58f115`, 2026-07-18
- **Reviewed**: 2026-07-18 at `4281d2c` (review-plan pass) — no drift
  in ci.yml; all seven `uses:` lines confirmed at the quoted line
  numbers (25/26/29/35/43/44/61), five distinct actions,
  `.github/dependabot.yml` confirmed absent, comment convention
  (ci.yml:13-14) confirmed. No content changes needed

## Why this matters

The repo SHA-pins its one git Cargo dependency (`tauri-nspanel` at a
full 40-char rev in `src-tauri/Cargo.toml`) — but its CI resolves five
actions from mutable refs at run time, including
`dtolnay/rust-toolchain@stable`, which is a moving *branch*, not even a
version tag. A compromised upstream tag/branch would execute in CI with
the `GITHUB_TOKEN` (`contents: read`, `checks: write`). Blast radius is
limited (the workflow triggers on `pull_request`, not
`pull_request_target`, and the repo has few secrets), but the posture
is inconsistent between the two build surfaces, and SHA-pinning is
cheap. A Dependabot config keeps the pins from rotting.

## Current state

`.github/workflows/ci.yml` — the five mutable `uses:` refs (line
numbers at `a58f115`):

```
25:      - uses: actions/checkout@v4
26:      - uses: dtolnay/rust-toolchain@stable
29:      - uses: Swatinem/rust-cache@v2
35:      - uses: rustsec/audit-check@v2.0.0        # version tag, still mutable
43:      - uses: actions/checkout@v4
44:      - uses: actions/setup-node@v4
61:      - uses: actions/checkout@v4
```

(`rustsec/audit-check@v2.0.0` is version-pinned but a tag can still be
force-moved; include it in the sweep.)

There is no `.github/dependabot.yml` in the repo.

Convention note: workflow comments in this repo are lowercase and
explain constraints (see the header comment at ci.yml:13-14) — match
that style.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Resolve a tag to a SHA | `git ls-remote https://github.com/actions/checkout v4` (and each repo/ref) | prints `<sha>\trefs/tags/v4` |
| Alternative resolver | `gh api repos/actions/checkout/git/ref/tags/v4 --jq .object.sha` | prints the SHA (deref annotated tags: if `.object.type` is `tag`, follow with `gh api repos/<r>/git/tags/<sha> --jq .object.sha`) |
| YAML sanity | `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))"` | exit 0 (if PyYAML missing, use `npx --yes yaml lint` or report) |
| Full local gate (unchanged by this plan) | `just test-all` | all green |

## Scope

**In scope** (the only files you should modify):
- `.github/workflows/ci.yml` (the `uses:` lines and adjacent comments only)
- `.github/dependabot.yml` (create)
- `plans/README.md` (status row)

**Out of scope** (do NOT touch, even though they look related):
- Any `run:` step, job structure, trigger, or permissions block in
  ci.yml — plan 007 already hardened those; this plan is `uses:` lines
  only.
- `src-tauri/Cargo.toml` / lockfiles.
- Enabling Dependabot for npm/cargo ecosystems — CI already runs
  `npm audit` and `rustsec/audit-check`; adding version-bump PRs for
  those ecosystems is a separate decision the maintainer has not made.

## Git workflow

- Branch: `advisor/029-ci-action-sha-pinning`.
- Commit style: `ci: pin actions to commit shas; dependabot keeps them fresh`.
- Do NOT push or open a PR unless the operator instructed it (note:
  this plan's full verification NEEDS a CI run, which needs a push —
  see Done criteria; ask the operator rather than pushing yourself).

## Steps

### Step 1: Resolve each ref to a full commit SHA

For each of: `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`,
`Swatinem/rust-cache@v2`, `rustsec/audit-check@v2.0.0`,
`actions/setup-node@v4` — resolve the ref via `git ls-remote` (tags may
be annotated: `git ls-remote` may show both `refs/tags/v4` and
`refs/tags/v4^{}`; use the `^{}` peeled SHA when present). For
`dtolnay/rust-toolchain@stable`, resolve the `stable` *branch* head:
`git ls-remote https://github.com/dtolnay/rust-toolchain refs/heads/stable`.

Record each `(action, human ref, sha)` triple.

**Verify**: five 40-hex-char SHAs recorded; each command exited 0.

### Step 2: Rewrite the `uses:` lines

Format (keep the human-readable ref as a trailing comment — that is
what Dependabot updates and humans read):

```yaml
      - uses: actions/checkout@<sha>  # v4
      - uses: dtolnay/rust-toolchain@<sha>  # stable branch, pinned (plan 029)
```

All seven `uses:` occurrences (three are `actions/checkout`— same SHA).

**Verify**: `grep -n "uses:" .github/workflows/ci.yml` → every line
matches `@[0-9a-f]{40}` with a `# <ref>` comment;
`python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))"` → exit 0.

### Step 3: Add `.github/dependabot.yml`

```yaml
# keeps the sha-pinned actions in ci.yml current (plan 029) — dependabot
# updates the pin and the trailing version comment together.
version: 2
updates:
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: monthly
```

**Verify**: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/dependabot.yml'))"` → exit 0.

## Test plan

No code paths change; the workflow itself is the artifact. Local
verification is the YAML parse + grep. The definitive check — a green
CI run with the pinned SHAs — requires a push, which is the operator's
call; record it as the pending gate in the status row (precedent: plan
007 did exactly this, "pending CI gate" noted in `plans/README.md`
until the next push went green).

## Done criteria

Machine-checkable. ALL must hold:

- [ ] Every `uses:` in ci.yml is `owner/repo@<40-hex-sha> # <ref>` (grep above)
- [ ] `.github/dependabot.yml` exists, parses, covers `github-actions`
- [ ] No `run:`/trigger/permissions line in ci.yml changed (`git diff .github/workflows/ci.yml` shows only `uses:` hunks + the new file)
- [ ] `plans/README.md` status row updated (DONE pending first green CI run on push, operator-owed)

## STOP conditions

Stop and report back (do not improvise) if:

- No network access (`git ls-remote` fails) — the plan cannot proceed;
  do not guess SHAs from memory or training data under any
  circumstances.
- A resolved SHA looks wrong (ls-remote returns nothing for the ref) —
  the upstream may have restructured tags; report what you found.
- You are tempted to "also" bump an action's major version — pin the
  version currently in use; upgrades are Dependabot's job later.

## Maintenance notes

- Dependabot PRs will now arrive for action bumps; the maintainer
  reviews the upstream diff before merging — that review IS the point
  of pinning.
- If the operator prefers to accept mutable tags instead (a defensible
  call for a single-maintainer repo), reject this plan in
  `plans/README.md`'s rejected-findings section so it is not re-audited.
- `dtolnay/rust-toolchain` pinned off `stable` still installs the
  *current* stable rust (the pin freezes the action's code, not the
  toolchain) — no reproducibility change to builds.
