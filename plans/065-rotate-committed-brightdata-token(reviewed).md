# Plan 065: Rotate and de-commit the hardcoded BrightData API token

> **Executor instructions**: Follow this plan step by step. This plan
> touches credential material — re-read Hard Rule 4 before starting: never
> print, log, or commit the actual token value anywhere, including in your
> own command output, commit messages, or status updates. Reference it only
> as "the BrightData API token" or by its file:line location. When done,
> update the status row for this plan in `plans/README.md` — unless a
> reviewer dispatched you and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- mcp-servers/ .mcp.json`
> If the mcp-servers tree or `.mcp.json` changed since this plan was
> written, re-verify the token is still present at the cited locations
> before proceeding; on a mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW (mechanical secret relocation) — but the rotation step
  itself is operator-owed and outside this plan's automatable scope
- **Depends on**: none
- **Category**: security
- **Planned at**: commit `f6c2f46`, 2026-07-20

## Why this matters

`mcp-servers/brightdata/mcp-config.json` and `mcp-servers/mcp-config-all.json`
both contain a live-looking BrightData API token hardcoded in an `env`
block, committed to git and tracked since commit `7430b4b` ("added mcp
plans and skills"). This is **not** part of the notchtap app itself (it's
local Claude Code / agent tooling configuration that happens to live in
the same repository) — but it's in this repo's git history, which means
anyone with clone access (or anyone who ever gains it, including a future
public fork) can read the token. A committed secret is compromised the
moment it's committed, regardless of whether it's later deleted from the
working tree — history retains it. This repo already has a precedent for
exactly this class of fix: plan 006 (Telegram token redaction) and plan
005 (OpenRouter key relocation + rotation) both treated "credential in a
place git can see it" as needing both removal *and* rotation, not removal
alone.

This repo already has a documented pattern for keeping local secrets out
of git: `secrets.toml` at `~/.config/notchtap/` (mode `0600`, referenced
via a `{file:...}` indirection from tracked config, per `opencode.json`'s
precedent recorded in `CLAUDE.md`). The same shape applies here.

## Current state

- `mcp-servers/brightdata/mcp-config.json` (repo root) — a tracked JSON
  file with this shape (token redacted per Hard Rule 4; confirm the exact
  key name yourself when you open the file):

  ```json
  {
    "mcpServers": {
      "brightdata": {
        "command": "node",
        "args": ["/Users/chetanjain/Desktop/Claude/ai-setup/mcp-servers/brightdata/brightdata-mcp-wrapper.js"],
        "env": {
          "API_TOKEN": "<the token — do not print or copy this value anywhere>",
          "WEB_UNLOCKER_ZONE": "mcp_unlocker",
          "BROWSER_ZONE": "mcp_browser"
        }
      }
    }
  }
  ```

- `mcp-servers/mcp-config-all.json` — contains the same token TWICE:
  line 9 as `API_TOKEN` and line 20 as `BRIGHTDATA_API_KEY` (verified at
  review, 2026-07-21 — the original "1 match in each file" count was
  wrong because it grepped only for `API_TOKEN`; the token also travels
  under the env-var name `BRIGHTDATA_API_KEY`).
- **Three more tracked locations the original plan missed** (verified
  2026-07-21 — all hold the *same* 36-character UUID-format token value,
  all introduced in the same commit `7430b4b`):
  - `mcp-servers/brightdata/test-tools.js:6` — `API_TOKEN` hardcoded in
    a spawn `env` block of a test script.
  - `mcp-servers/product-research-plugins/test-init.js:6` —
    `BRIGHTDATA_API_KEY` hardcoded in a spawn `env` block.
  - `.mcp.json` (repo root) lines 13 and 24 — `API_TOKEN` and
    `BRIGHTDATA_API_KEY`. This is the file Claude Code actually loads
    for project-scoped MCP servers, so it is almost certainly the *live*
    consumer (Step 1's "check what reads them" question) — the
    `mcp-servers/*.json` copies may be reference duplicates.
  Total: 5 tracked files, 7 occurrences, one token value. Any grep used
  in this plan must match **both** env-var names:
  `grep -rnE "API_TOKEN|BRIGHTDATA_API_KEY"` — an `API_TOKEN`-only
  pattern misses two occurrences.
- Neither file is gitignored: `git check-ignore -v mcp-servers/brightdata/mcp-config.json`
  returned nothing (exit 1) at planning time, and `git log --oneline -- mcp-servers/brightdata/mcp-config.json`
  shows it tracked since `7430b4b`.
- `.gitignore` (repo root) already has precedent entries for local/secret
  material — check it for the pattern used (e.g. how `secrets.toml` or
  similar local-only files are excluded) and match that style for the new
  ignore entries this plan adds.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Confirm tracked status | `git ls-files .mcp.json mcp-servers/brightdata/mcp-config.json mcp-servers/mcp-config-all.json mcp-servers/brightdata/test-tools.js mcp-servers/product-research-plugins/test-init.js` | all five paths listed |
| Confirm token count | `grep -rcE "API_TOKEN\|BRIGHTDATA_API_KEY" .mcp.json mcp-servers/brightdata/mcp-config.json mcp-servers/mcp-config-all.json mcp-servers/brightdata/test-tools.js mcp-servers/product-research-plugins/test-init.js` | matches what you find when you open the files (at review time: 2/1/2/1/1) |
| Verify ignore takes effect | `git status --short` after Step 2 | the now-untracked-content files show as unmodified/ignored, not staged |

## Scope

**In scope**:
- `mcp-servers/brightdata/mcp-config.json`
- `mcp-servers/mcp-config-all.json`
- `mcp-servers/brightdata/test-tools.js` (added at review 2026-07-21 —
  hardcoded token in its spawn `env`; switch it to reading
  `process.env`, matching the pattern its sibling
  `brightdata-mcp-wrapper.js` already uses)
- `mcp-servers/product-research-plugins/test-init.js` (added at review
  2026-07-21 — same treatment)
- `.mcp.json` (repo root — added at review 2026-07-21; likely the live
  loader config, so Step 1's indirection decision applies here first)
- `.gitignore` (repo root) — add entries so the token-bearing files (or a
  split-out local secrets file, per Step 1's decision) never get
  re-committed
- A new local, gitignored file to hold the actual token going forward
  (exact name/location is your call in Step 1 — follow this repo's
  `secrets.toml` precedent if a natural equivalent exists for MCP server
  env vars, otherwise a plain gitignored `.local.json` override)

**Out of scope**:
- Any file under `src-tauri/`, `src/`, or the notchtap app itself — this
  finding is unrelated to the shipped app.
- Any other MCP server config in `mcp-servers/` that doesn't contain
  `API_TOKEN` or similar credential material — don't touch what isn't
  broken.
- The actual token rotation (requesting a new token from BrightData) —
  that's an operator action outside this plan's automatable scope; see
  STOP conditions.

## Steps

### Step 0: STOP — confirm rotation authorization before touching anything

This plan's mechanical steps (Steps 1-3) only stop the *bleeding* — they
remove the token from future commits. They do **not** invalidate the
already-committed, already-compromised value, and rewriting git history
to purge it retroactively is a separate, much higher-risk operation this
plan does not include (force-push, coordination with anyone else who has
a clone, etc.) and should not be attempted without explicit operator
sign-off.

**Before Step 1**: tell the operator plainly that (a) the committed token
should be treated as burned and rotated at BrightData's dashboard
regardless of what this plan does to the working tree, and (b) purging
it from git history is a separate, larger decision this plan is
deliberately not making for them. If the operator hasn't acknowledged
this, STOP here and report back rather than proceeding — don't silently
do the file relocation and call the security issue closed.

### Step 1: Move the token to a gitignored file

Pick the shape that best matches how these two config files are consumed
(check what reads them — likely Claude Code's MCP server loader). If the
loader supports `{file:...}`-style indirection (like this repo's
`opencode.json` precedent), use that. Otherwise, split the `env` block's
sensitive keys into a new gitignored file (e.g.
`mcp-servers/brightdata/mcp-config.local.json`) and have the tracked file
either merge it at load time (only if the loader supports config
overlays) or, if it doesn't, leave a placeholder value plus a comment/
README note pointing at where the real value now lives, with the tracked
file itself no longer containing the working credential.

**Verify**: `grep -rnE "API_TOKEN|BRIGHTDATA_API_KEY" .mcp.json mcp-servers/ --include="*.json" --include="*.js" | grep -v node_modules` — the tracked files now contain no live token value under either env-var name (a placeholder string like `"<see mcp-config.local.json>"` or a `process.env` read is fine).

### Step 2: Add `.gitignore` entries

Add the new local file(s) from Step 1 to `.gitignore`, matching the
style of existing entries there.

**Verify**: `git status --short` shows the new local file as untracked-and-ignored (or doesn't show it at all), not staged.

### Step 3: Confirm the app-affecting files still work

If any tooling actually loads these MCP configs at session start, do a
sanity check that the new indirection resolves correctly — e.g. start a
fresh Claude Code session in this repo and confirm the `brightdata` MCP
server still connects. If you can't exercise this yourself, say so in
your completion report rather than claiming it works.

**Verify**: MCP server connects (manual/operator-owed if you can't drive it yourself).

## Test plan

No automated test suite covers `mcp-servers/` — this is config, not
application code. Verification is the grep-based checks in Steps 1-2
plus the manual connectivity check in Step 3.

## Done criteria

- [ ] `grep -rE "(API_TOKEN|BRIGHTDATA_API_KEY).{0,10}[0-9a-f]{8}-[0-9a-f]{4}" .mcp.json mcp-servers/ | grep -v node_modules` returns no matches in tracked files (the token is UUID-format — this pattern catches it under either env-var name without printing the full value)
- [ ] The new local secret file exists, is gitignored, and is NOT staged (`git status --short`)
- [ ] Operator has explicitly acknowledged the token needs rotation at BrightData's dashboard (Step 0) — record this acknowledgment in your completion note, don't just assume it
- [ ] No *source/config* files outside the Scope section modified (`git status` — `plans/README.md` is expected to change too; everything else is out of scope)
- [ ] `plans/README.md` status row for 065 updated, explicitly noting rotation is operator-owed and not yet confirmed done (mirror the phrasing style plans 005/007/029/045 use for "operator-owed" gates)

## STOP conditions

- The operator has not acknowledged the rotation requirement (Step 0) —
  do not proceed past this point without it.
- The token appears in more than the five files named in "Current state"
  (verified list as of 2026-07-21) — if a sixth
  location turns up, stop and report the full list before deciding scope,
  since Hard Rule 4 means you should not be pasting multiple locations'
  worth of secret context into a single sweeping fix without the operator
  seeing the full blast radius first.
- Anything in this plan would require you to print, log, or otherwise
  reproduce the token's actual value — refuse and report instead.

## Maintenance notes

- This is a git-history-level exposure, not just a working-tree one — if
  the operator later decides to purge history (e.g. via `git filter-repo`
  or BFG), that's a separate, carefully-scoped operation with its own
  blast-radius review (force-push implications, coordination with any
  other clones/forks) — don't fold it into a future "quick fix" without
  treating it with the same care as any other history rewrite.
- If other `mcp-servers/*.json` files gain credentials in the future,
  apply the same gitignored-local-file pattern from the start rather than
  discovering this finding again.

---

**Review-plan pass (2026-07-21)**: cold re-verification at HEAD `647f6d0`.
The token is STILL present and STILL tracked — nothing has been rotated or
de-committed since planning. Exposure is *wider* than the plan stated: the
same 36-character UUID-format token exists in **5 tracked files / 7
occurrences** (the plan named 2 files): `mcp-servers/brightdata/mcp-config.json:9`,
`mcp-servers/mcp-config-all.json:9` + `:20`, `mcp-servers/brightdata/test-tools.js:6`,
`mcp-servers/product-research-plugins/test-init.js:6`, and `.mcp.json:13` +
`:24` at repo root. Two of the seven occurrences use the env-var name
`BRIGHTDATA_API_KEY`, which the plan's original `API_TOKEN`-only greps
would have silently missed. All 7 entered in the single commit `7430b4b`
(the only commit touching these files), so history exposure is unchanged:
one commit, still reachable. None of the files is gitignored
(`git check-ignore` exit 1 re-confirmed). `node_modules` copies of README
examples are untracked (gitignored) — not part of the exposure. No OTHER
credential type was found in `mcp-servers/` or `.mcp.json` — every
credential-looking hit resolves to this one BrightData token. Edits
applied this pass: drift-check path widened to include `.mcp.json`;
"Current state" corrected to the full 5-file/7-occurrence list; Scope,
command table, Step 1 verify, Done-criteria grep, and the
third-location STOP condition all updated to cover both env-var names
and all five files. Step 0 (operator rotation acknowledgment gate) is
unchanged and still mandatory — the token must be treated as burned and
rotated at BrightData regardless of the file relocation.
