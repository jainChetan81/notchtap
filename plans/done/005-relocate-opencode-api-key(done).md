# Plan 005: Verify the relocated OpenRouter key and complete rotation

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in "STOP conditions" occurs, stop and report; do not
> improvise. This plan cannot be marked DONE until the human operator confirms
> rotation. When done, update the status row in `plans/README.md`.
>
> **Drift check (run first)**:
> `git diff --stat b1981c9..HEAD -- .gitignore plans/README.md`
> Also run `git diff --stat -- .gitignore plans/README.md` and
> `git diff --cached --stat -- .gitignore plans/README.md` to expose working-
> tree/index changes. The plan index may have unrelated status edits; any
> `.gitignore` change or conflict is a STOP. Then run Step 1: Git cannot detect
> ignored or home-directory drift, so its checks complete the gate.
>
> **CRITICAL - secret handling**: never print, echo, log, commit, paste into
> chat, or place either key value directly in a terminal command. Never run
> `opencode debug config`: current OpenCode prints the fully resolved config,
> including provider credentials.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: MED
- **Depends on**: none
- **Category**: security
- **Planned at**: commit `b1981c9`, 2026-07-17 (reviewed after relocation)

## Why this matters

The credential was previously stored literally in a world-readable ignored
file under the repository root. Relocation is now complete: both OpenCode
config files contain only a supported `{file:...}` reference, and the actual
credential is outside the repository with mode `0600`. The remaining security
action is rotation because filesystem permission repair cannot make a
previously exposed credential trustworthy again.

## Current state

Verified without reading the credential value on 2026-07-17:

- `opencode.json` is ignored, untracked, mode `0644`, and its
  `provider.openrouter.options.apiKey` value is exactly
  `{file:~/.config/opencode/openrouter.key}`.
- `~/.config/opencode/opencode.json` exists, is byte-identical to the project
  config, and contains the same file reference. Its `0644` mode is acceptable
  because it contains no credential value.
- `~/.config/opencode/openrouter.key` is a regular file, not a symlink, and is
  mode `0600`. Its contents were not read during review.
- `git log --all --oneline -- opencode.json` has no output. This proves only
  that this exact path is absent from reachable history; it is not a claim
  that every historical path has undergone secret scanning.
- Installed OpenCode is `1.18.3`. Current official config documentation
  supports project/global config merging and `{file:path}` substitution:
  <https://opencode.ai/docs/config/#locations> and
  <https://opencode.ai/docs/config/#variables>.
- `opencode debug info --pure` loads the resolved configuration but does not
  serialize provider options in OpenCode 1.18.3. It is the safe load check used
  below; `opencode debug config` is explicitly unsafe for this task.

The duplicate project/global configuration is not itself a security issue and
must not be reorganized in this plan. Dedupe only in a separate tooling change
after deciding whether these model/MCP settings should be global or
repository-specific.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Installed version | `opencode --version` | `1.18.3` (or STOP if changed) |
| Project reference | `jq -e '.provider.openrouter.options.apiKey == "{file:~/.config/opencode/openrouter.key}"' opencode.json >/dev/null` | exit 0, no output |
| Global reference | `jq -e '.provider.openrouter.options.apiKey == "{file:~/.config/opencode/openrouter.key}"' "$HOME/.config/opencode/opencode.json" >/dev/null` | exit 0, no output |
| Credential metadata | `test -s "$HOME/.config/opencode/openrouter.key" && test -f "$HOME/.config/opencode/openrouter.key" && test ! -L "$HOME/.config/opencode/openrouter.key" && test "$(stat -f '%Lp' "$HOME/.config/opencode/openrouter.key")" = 600` | exit 0, no output |
| Config load | `opencode debug info --pure >/dev/null` | exit 0, no output |
| Exact-path history | `git log --all --oneline -- opencode.json` | no output |
| Ignore/index guard | `git check-ignore -q opencode.json && ! git ls-files --error-unmatch opencode.json >/dev/null 2>&1` | exit 0, no output |

## Scope

**In scope** (the only files that may be modified):

- `~/.config/opencode/openrouter.key` - human operator replaces the credential
  value, then restores mode `0600`.
- `plans/README.md` - mark this plan DONE only after operator confirmation.

**Read-only verification targets** (do not modify):

- `opencode.json`
- `~/.config/opencode/opencode.json`
- `.gitignore`

**Out of scope**:

- Moving or deleting either OpenCode JSON config, changing model/MCP settings,
  or switching to environment-variable storage.
- `.gitignore`; retain the ignore rule as defense in depth.
- `src-tauri/**`, `src/**`, and the app's separate
  `~/.config/notchtap/secrets.toml` handling.
- Git-history rewriting. If history evidence appears, rotation remains urgent,
  but remediation requires a separately approved plan.

## Git workflow

- Do not create a branch or commit unless the dispatcher/operator explicitly
  requests it. The credential file is outside Git; only the plan-index status
  is eligible for a later documentation commit.
- Never stage `opencode.json`, including with `git add -f`.
- Do not push or open a PR.

## Steps

### Step 1: Confirm the relocation is still intact

Before changing anything, capture the pre-existing repository status excluding
the one index file this plan may update:

```sh
status_before="$(mktemp)"
git status --porcelain=v1 | rg -v '^.. plans/README\.md$' > "$status_before" ||
  test $? -eq 1
```

Keep that path in the same shell through Step 4. Run every command in the table
above, then confirm the two non-secret config files remain identical:

```sh
cmp -s opencode.json "$HOME/.config/opencode/opencode.json"
```

**Verify**: every command exits 0 with no credential-bearing output, and the
history command produces no output. If any check fails, STOP; do not inspect a
credential by printing it.

### Step 2: Have the human operator rotate the credential

This is an operator-only dashboard action. An automated executor dispatched
without the operator present must change plan 005's index status to
`BLOCKED (operator rotation pending)` and report `relocation verified;
OpenRouter rotation requires the operator`. Do not continue or mark DONE.

The operator must:

1. Create a replacement key in the OpenRouter dashboard.
2. In an interactive zsh with shell tracing disabled, run the exact function
   below. It reads without terminal echo, writes a `0600` same-directory temp
   file under `umask 077`, and atomically replaces the credential file. The
   value never appears in the command text or process arguments:

   ```zsh
   rotate_openrouter_key() {
     local replacement tmp
     umask 077
     tmp="$(mktemp "$HOME/.config/opencode/.openrouter.key.XXXXXX")" || return 1
     printf 'New OpenRouter key: ' >&2
     IFS= read -r -s replacement
     printf '\n' >&2
     if [[ -z "$replacement" ]]; then
       rm -f "$tmp"
       return 1
     fi
     printf '%s\n' "$replacement" > "$tmp" || { rm -f "$tmp"; return 1; }
     chmod 600 "$tmp" || { rm -f "$tmp"; return 1; }
     mv -f "$tmp" "$HOME/.config/opencode/openrouter.key"
   }
   rotate_openrouter_key && unfunction rotate_openrouter_key
   ```

   Do not run this under `set -x`; type or use a trusted password-manager paste
   for the hidden prompt, and clear any clipboard history afterward.
3. Re-run the credential metadata check from the command table.
4. Run the Step 3 checks.
5. Run the authenticated no-generation smoke check below. It passes the
   authorization header to curl through stdin rather than argv and discards the
   response body:

   ```sh
   status="$(
     {
       printf 'url = "https://openrouter.ai/api/v1/key"\n'
       printf 'header = "Authorization: Bearer '
       tr -d '\r\n' < "$HOME/.config/opencode/openrouter.key"
       printf '"\n'
       printf 'silent\nshow-error\noutput = "/dev/null"\nwrite-out = "%%{http_code}"\n'
     } | curl --disable --config -
   )"
   test "$status" = 200
   unset status
   ```

6. Revoke the old key in the OpenRouter dashboard only after the smoke check
   exits 0. `GET /api/v1/key` validates the key without model generation; see
   <https://openrouter.ai/docs/api_reference/limits>.

Do not put either key value into this plan or `plans/README.md`. Record only the
operator confirmation date.

**Verify**: the metadata check and authenticated smoke check exit 0, then the
operator confirms that the old key was revoked. Without all three, mark the
plan `BLOCKED (operator rotation pending)` rather than DONE.

### Step 3: Re-check storage and config loading

Run, in order:

```sh
test -s "$HOME/.config/opencode/openrouter.key"
test -f "$HOME/.config/opencode/openrouter.key"
test ! -L "$HOME/.config/opencode/openrouter.key"
test "$(stat -f '%Lp' "$HOME/.config/opencode/openrouter.key")" = 600
opencode debug info --pure >/dev/null
```

These commands check presence, type, mode, and config resolution without
printing the value. The authenticated Step 2 check separately proves that
OpenRouter accepts the replacement.

**Verify**: all five commands exit 0 with no output.

### Step 4: Update the plan index

After, and only after, the operator confirms replacement plus revocation,
change plan 005's status in `plans/README.md` from `TODO` to `DONE`.

Confirm this execution did not alter any other repository path, then remove the
temporary status file:

```sh
status_after="$(mktemp)"
git status --porcelain=v1 | rg -v '^.. plans/README\.md$' > "$status_after" ||
  test $? -eq 1
diff -u "$status_before" "$status_after"
rm -f "$status_before" "$status_after"
```

**Verify**:
the `diff` has no output, and
`rg '^\| 005 .*\| DONE \|$' plans/README.md` has exactly one matching row.

## Test plan

No application tests are needed because no application code changes. The
verification is configuration-specific:

- both JSON configs resolve only through the approved external file reference;
- the external credential file is non-empty, regular, non-symlink, and `0600`;
- OpenCode 1.18.3 loads the config through `debug info --pure`;
- OpenRouter returns HTTP 200 from the authenticated no-generation endpoint;
- the operator confirms old-key revocation.

## Done criteria

- [ ] Both no-output `jq -e` checks exit 0.
- [ ] The credential metadata check exits 0 and reports no value.
- [ ] `opencode debug info --pure >/dev/null` exits 0 after replacement.
- [ ] The authenticated `GET /api/v1/key` smoke check returns HTTP 200 without
      printing or placing the credential in argv.
- [ ] `git log --all --oneline -- opencode.json` has no output.
- [ ] Human operator confirms the replacement key works and the old key is
      revoked; no credential value or suffix is recorded.
- [ ] No file outside the in-scope list was modified.
- [ ] `plans/README.md` marks 005 DONE only after all prior criteria hold.

## STOP conditions

- Either JSON config no longer contains exactly the approved file reference.
- The external credential path is missing, empty, a symlink, or not mode
  `0600`.
- OpenCode is no longer `1.18.3`, or `opencode debug info --pure` fails. Re-vet
  the safe diagnostic before using a new version; never fall back to
  `opencode debug config`.
- `git log --all --oneline -- opencode.json` produces output.
- Rotation would require printing a key, putting it in shell history, or
  modifying an out-of-scope file.
- The operator cannot confirm both replacement-key use and old-key revocation.
- Shell tracing is enabled, secure hidden input is unavailable, curl cannot
  read config from stdin, or the authenticated check returns a non-200 status.

## Maintenance notes

- Keep provider credentials outside repositories and reference them through
  OpenCode's supported `{file:...}` substitution. Config files containing only
  the reference do not themselves need mode `0600`; the referenced credential
  file does.
- Reviewers should reject any completion update that says only "relocated".
  Relocation is already complete; rotation is the remaining risk treatment.
- A future OpenCode upgrade must re-check that the chosen config diagnostic
  remains non-secret before using it in automation.
