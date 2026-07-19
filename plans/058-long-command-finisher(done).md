# Plan 058: `notchtap run` — long-command finisher (wrap a command, push on completion)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f58ced2..HEAD -- notchtap`
> On any change, compare the excerpt below; mismatch = STOP.

## Status

- **Priority**: P3 (operator-requested, filed directly — not from an
  audit or spike session; same precedent as
  `004-test-notifications(done).md` / `005-appearance-config(done).md`.
  **Review-plan correction**: the field previously held "Feature," not a
  P-value — every other plan in this repo uses P1/P2/P3 here, and
  `plans/README.md`'s own dependency notes rely on that being sortable;
  fixed to P3, matching this plan's own low urgency/low risk profile.
  Same correction applied to `plans/README.md`'s status row.)
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: feature
- **Planned at**: commit `f58ced2`, 2026-07-20
- **Review-plan pass (2026-07-20)**: verified live against `notchtap`
  (zero drift since `f58ced2`) and `http.rs`'s `NotifyRequest`
  (`priority`/`details` confirmed real optional wire fields — no
  server-side change needed, claim accurate). Traced the proposed Step 1
  shell logic under `set -u` semantics and found two real gaps, both
  fixed in Step 1 below: (1) every one of `run`'s own flag-validation
  failures called the shared top-level `usage()`, which only prints the
  `--title`/`--body` flags-mode message — a typo in `notchtap run
  --min-secs` would show the user an error describing a completely
  different command mode; (2) `--min-secs` was the one `run` flag with
  no input validation (contrast `--priority-success`/`--priority-failure`,
  which both `case`-validate), so a non-numeric value would crash with a
  shell arithmetic error inside `[ ... -lt ... ]` instead of a clean
  `usage` exit. Two narrower, non-blocking observations folded into
  Maintenance notes instead of fixed: a literal `cd` as the wrapped
  command could break the `"$0"` self-invocation if `$0` is a relative
  path (narrow — most wrapped commands are build tools, not shell
  builtins); Ctrl-C during the wrapped command likely kills the whole
  script before the completion push fires (probably fine — being at the
  terminal to press Ctrl-C means you don't need the "it's done" nudge).

## Why this matters

Surveyed against every other "not yet built" idea on record (github
watcher, calendar, kharcha, focus sync — `plans/README.md`'s Direction
options and this repo's design-doc spikes), this is the one with **zero
new config surface**: no OAuth, no token storage, no new `SourceKind`,
no new `config.toml` table, no settings-window section. Every knob is a
CLI flag, not a persisted setting. It also delivers value immediately —
wrap any long-running command and stop babysitting the terminal for it
to finish.

## Current state

`notchtap` (repo root) is a flags-only POSIX `sh` script (spec §12,
plan 035's rich-relay amendment). Relevant structure today:

```sh
#!/bin/sh
# notchtap — cli push for the notchtap notification engine (spec §12).
# flags only, no positional form:
#   notchtap --title <title> --body <body> [--subtitle <s>] [--detail Label=Value]... [--priority low|medium|high] [--signal <s>] [--source cmux] [--port <p>]
...
set -u

usage() {
  echo "usage: notchtap --title <title> --body <body> [...]" >&2
  exit 2
}

title=""
body=""
...
port="${NOTCHTAP_PORT:-9789}"
...
while [ $# -gt 0 ]; do
  case "$1" in
    --title) ... ;;
    ...
    *)
      usage ;;
  esac
done

[ -n "$title" ] || usage
[ -n "$body" ] || usage
...
payload=$(jq -n ... )
status=$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' \
  --max-time 5 --request POST "http://127.0.0.1:${port}/notify" \
  --header 'content-type: application/json' --data "$payload") || exit 1
case "$status" in
  2*) exit 0 ;;
  *) echo "notchtap: server answered $status" >&2; exit 1 ;;
esac
```

Today, any first argument that isn't a recognized flag falls through to
`*) usage` and exits 2 — so `notchtap run ...` currently just errors.
`http.rs`'s `NotifyRequest` already accepts `priority`/`details` per
request (plan 035); no server-side change is needed for this feature —
it composes entirely from the existing wire contract via a **recursive
self-invocation** (`"$0" --title ... --detail ...`), reusing the
existing jq/curl push code with zero duplication and zero drift risk
between two push code paths.

## Scope

**In scope**:
- `notchtap` (repo root) — add a `run` subcommand, dispatched before the
  existing flags loop
- `README.md` — one usage-list bullet
- `plans/README.md` — status row

**Out of scope**:
- Any Rust/settings/config.toml change. No new `SourceKind`, no new
  `rotation_order` entry, no settings-window section — this is
  deliberately CLI-only (per-invocation flags, never persisted).
- Capturing or forwarding the wrapped command's stdout/stderr into the
  notification. Two reasons: (1) command output can carry secrets or
  tokens — this repo has an established discipline against echoing
  untrusted/sensitive output into a notification (`notifier.rs`'s
  telegram-token redaction, `settings.rs`'s malformed-toml-detail
  withholding); (2) capturing would break interactive/TUI commands that
  expect a real tty. Only the exit code and elapsed time are reported.
- New priority tiers beyond the existing `low|medium|high`.
- A `docs/recipes/long-command-finisher.md` write-up — optional
  nice-to-have, not required for Done criteria (mirrors
  `docs/recipes/kuma-webhook.md`'s format if the executor has time).

## Git workflow

- Current branch (or a worktree branch, e.g. `exec/054-long-command-finisher`,
  matching this repo's convention for larger plans — either is fine at
  this Effort/Risk).
- Commit message: `cli: notchtap run — long-command finisher subcommand`.
- Do NOT push — operator merges per their own workflow (012's precedent).

## Steps

### Step 1: Add the `run` subcommand

Insert immediately **after** the `usage()` function definition and
**before** the `title=""` variable block. This is the script's one
deliberate exception to "flags only, no positional form" — say so
explicitly in the header comment (Step 1 also amends the top-of-file
comment block to document `run` as a second invocation mode, distinct
from the flags-only push mode).

```sh
# notchtap run — the one deliberate exception to "flags only": wraps a
# command, times it, and pushes a completion notification via a
# self-invocation of this same script's flags path below (zero
# duplicated push logic, so the two modes can never drift apart).
#   notchtap run [--label <name>] [--min-secs <n>]
#     [--priority-success low|medium|high] [--priority-failure low|medium|high]
#     [--port <p>] -- <command...>
# exit code is ALWAYS the wrapped command's own — a failed push never
# changes it (best-effort only; a push failure warns to stderr).
# a fast, successful run is suppressed below --min-secs (default 15s);
# a failure always pushes regardless of duration — a fast failure is
# exactly the "did something break while I looked away" case a slow one
# isn't.
run_usage() {
  echo "usage: notchtap run [--label <name>] [--min-secs <n>] [--priority-success low|medium|high] [--priority-failure low|medium|high] [--port <p>] -- <command...>" >&2
  exit 2
}

if [ "${1:-}" = "run" ]; then
  shift
  run_label=""
  run_min_secs=15
  run_priority_success="medium"
  run_priority_failure="high"
  run_port="${NOTCHTAP_PORT:-9789}"

  while [ $# -gt 0 ]; do
    case "$1" in
      --label)
        [ $# -ge 2 ] || run_usage
        run_label="$2"; shift 2 ;;
      --min-secs)
        [ $# -ge 2 ] || run_usage
        # must be a non-negative integer — it feeds a `[ -lt ]` numeric
        # comparison below, which errors out on non-numeric input rather
        # than failing cleanly.
        case "$2" in
          ''|*[!0-9]*) run_usage ;;
        esac
        run_min_secs="$2"; shift 2 ;;
      --priority-success)
        [ $# -ge 2 ] || run_usage
        case "$2" in low|medium|high) run_priority_success="$2" ;; *) run_usage ;; esac
        shift 2 ;;
      --priority-failure)
        [ $# -ge 2 ] || run_usage
        case "$2" in low|medium|high) run_priority_failure="$2" ;; *) run_usage ;; esac
        shift 2 ;;
      --port)
        [ $# -ge 2 ] || run_usage
        run_port="$2"; shift 2 ;;
      --)
        shift; break ;;
      *)
        run_usage ;;
    esac
  done
  [ $# -ge 1 ] || run_usage   # at least one token must follow --

  [ -n "$run_label" ] || run_label="$*"

  run_start=$(date +%s)
  "$@"
  run_status=$?
  run_end=$(date +%s)
  run_elapsed=$((run_end - run_start))

  if [ "$run_status" -eq 0 ] && [ "$run_elapsed" -lt "$run_min_secs" ]; then
    exit "$run_status"
  fi

  if [ "$run_elapsed" -ge 60 ]; then
    run_dur="$((run_elapsed / 60))m $((run_elapsed % 60))s"
  else
    run_dur="${run_elapsed}s"
  fi

  if [ "$run_status" -eq 0 ]; then
    run_title="Command finished"
    run_priority="$run_priority_success"
  else
    run_title="Command failed"
    run_priority="$run_priority_failure"
  fi

  "$0" --title "$run_title" --body "$run_label" \
    --detail "Exit=$run_status" --detail "Duration=$run_dur" \
    --priority "$run_priority" --port "$run_port" \
    || echo "notchtap run: push failed (command's own exit code is still authoritative)" >&2

  exit "$run_status"
fi
```

**Verify**: `sh -n notchtap` → exit 0 (this is the same command CI's
`web` job already runs — no CI file change needed).

### Step 2: README.md

Add one bullet to the `## what it does` list, near the existing
`notchtap --title ... --body ...` line:

```markdown
- wraps long-running commands and pushes a completion card when they
  finish: `notchtap run -- pnpm build` (skips the push for fast,
  successful runs; a failure always pushes)
```

### Step 3: Manual smoke (operator, same class as every CLI-adjacent
plan in this repo — `sh -n` is the automated proof, this is the live
check)

With `npm run tauri dev` running:

1. `notchtap run -- sleep 1` → no push, exits 0
2. `notchtap run --min-secs 0 -- sleep 1` → medium-priority push,
   body = `sleep 1`, details `Exit=0` / `Duration=1s`
3. `notchtap run -- false` → high-priority push even though it was
   instant (`Exit=1`)
4. `notchtap run --label "prod build" -- pnpm build` → the card's body
   reads `prod build`, not the raw command
5. Stop `npm run tauri dev` (server down), then
   `notchtap run --min-secs 0 -- true` → stderr shows the push-failed
   warning, but the shell's own `$?` after the command is still `0`
6. `notchtap run -- false && echo should-not-print` → `should-not-print`
   never prints (confirms the wrapped exit code propagates through a
   `&&` chain correctly)
7. `notchtap run --min-secs abc -- true` → exits 2, stderr shows the
   `run`-specific usage line (mentions `--label`/`--min-secs`/etc.), NOT
   the top-level `--title`/`--body` usage line — this is the
   review-plan-pass fix: before it, any `run`-mode flag mistake printed
   the wrong command's usage text

### Step 4: `plans/README.md`

Flip this plan's status row to DONE, one line, per every other plan's
convention.

## Test plan

No Rust or frontend change — no `cargo test`/`vitest` count updates.
Coverage is `sh -n` (already CI-gated, same command the `web` job runs
today) plus the Step 3 manual smoke — this mirrors the accepted testing
gap already on record for the `notchtap` script itself (no functional
shell-script test harness exists in this repo; `sh -n` + manual smoke is
the established pattern, not a new exception).

## Done criteria

- [ ] `grep -c '"run"' notchtap` → ≥1 (the dispatch condition)
- [ ] `sh -n notchtap` exits 0
- [ ] Step 3's seven manual cases all behave as described (operator-owed)
- [ ] the wrapped command's own exit code is what `notchtap run` returns
      in every case, including notify-server-down (case 5) and a `&&`
      chain (case 6)
- [ ] a bad `run`-mode flag (case 7) prints `run`'s own usage text, not
      the top-level flags-mode usage text
- [ ] README.md gains the one-line `notchtap run` mention
- [ ] `plans/README.md` status row updated

## STOP conditions

- The `usage()` function, the flags loop, or the jq/curl push block at
  the tail of the script no longer matches the quoted current-state
  excerpt above (reconcile by reading; STOP only if the push mechanism
  itself — payload construction or the exit-code handling on the
  `curl`/`status` result — changed in a way that breaks the
  "self-invoke `$0` with flags" reuse).
- `date +%s` or `$(( ))` arithmetic behaves unexpectedly on the target
  shell (extremely unlikely on macOS `/bin/sh`; flag if seen rather than
  working around it silently).

## Maintenance notes

- **Two narrow edge cases identified in review, not fixed (accepted as-is)**:
  (1) if the wrapped command is a literal shell builtin like `cd`
  (e.g. `notchtap run -- cd /somewhere`, unusual but possible) rather
  than an external command, it runs in the script's own shell — a `cd`
  would change the script's CWD, and if `$0` is a relative path (e.g.
  invoked as `./notchtap`), the later `"$0"` self-invocation could then
  fail to find the script or find the wrong file. Realistic wrapped
  commands are build/test tools, not raw shell builtins, so this is low
  likelihood — revisit only if it's ever actually hit. (2) Ctrl-C during
  the wrapped command likely delivers SIGINT to the whole foreground
  process group (script included, since it's not run in its own process
  group), killing `notchtap run` before it reaches the completion-push
  code — so a manually-interrupted command never gets a push. This
  arguably doesn't matter (being at the terminal to press Ctrl-C means
  you're already watching, so the "it's done" nudge isn't needed), but
  it's worth knowing this isn't the same as "a failure always pushes" —
  that guarantee only covers the wrapped command exiting non-zero on its
  own, not the script itself being killed.
- Any future "notify on completion" wrapper should reuse this same
  self-invocation pattern (`"$0" --title ... --detail ...`) rather than
  duplicating the jq/curl push logic a second time.
- The fast-success-suppressed / fast-failure-always-pushes asymmetry is
  deliberate — do not "simplify" it to a single duration check without
  re-reading "Why this matters."
- If a github-watcher or CI-status source is ever built (still
  unplanned, `plans/README.md`'s Direction options), it is a distinct
  feature (polls a remote API, needs its own config) — this plan does
  not evolve into that; it stays a local subprocess wrapper.
