---
name: delegate-to-kimi
description: delegate a bounded coding task to the kimi cli as a pure execution agent — claude plans the work, kimi codes it, claude verifies it. trigger with a one-line prompt like "delegate this to kimi."
---

# delegate-to-kimi

claude is the manager, kimi (`~/.kimi-code/bin/kimi`) is the executor: it
writes code against a plan claude already made, decides nothing about scope,
and its "done" report is never trusted on its own. kimi is an intent
interpreter, not a mind reader — vague instructions get guessed at, so the
plan carries the precision.

## the two rules that matter

1. **kimi only ever touches a scratch copy, inside a scoped `--add-dir` that
   lives entirely under `/tmp/kimi-delegation-tmp/<task>/`.** never the real
   kharcha db, never repo-root access, never `$HOME`, `.env*`, keychains,
   `.ssh`, or any other credential store, never a path outside that task's
   tmp directory. if kimi needs something outside the scoped dir — a
   cross-directory import, a config file two levels up — claude copies it in
   or inlines the relevant context in the plan; kimi never reaches for it
   itself. kimi runs via `-p`, which is non-interactive: no permission
   prompt, so inside whatever directory it's given it reads and writes
   freely — functionally "yolo," just scoped. that named directory is the
   entire boundary of what's allowed, no matter what the plan says. if a
   task can't be done inside a scoped directory, that's a sign it needs
   claude directly, not kimi.
2. **claude verifies everything after, independently.** re-run
   typecheck/lint, query the scratch db directly and check constraints (not
   just row counts), read the full `git diff` yourself, scan for unexpected
   changes (lockfiles, postinstall scripts, dotfiles, launchd plists), grep
   the log for anything sensitive, check for orphaned processes. kimi's
   self-report is never sufficient — this step alone has caught three real
   bugs kimi missed.

## workflow

1. **claude writes the plan.** what exists, the exact scratch source, the
   exact `--add-dir` scope (a subdirectory of
   `/tmp/kimi-delegation-tmp/<task>/`), what's out of scope, order of
   operations, how it'll be verified. name exact file paths, not
   descriptions — "the auth file" gets guessed at, `lib/auth/session.ts`
   doesn't. state whether kimi may install dependencies or needs network
   access; don't leave that as an assumption. write to
   `/tmp/kimi-delegation-tmp/<task>/plan.md`, post it in chat before
   launching anything — check it against rule 1 yourself before you do.
2. **launch:**
   ```
   mkdir -p /tmp/kimi-delegation-tmp/<task>
   kimi -p "read the plan at /tmp/kimi-delegation-tmp/<task>/plan.md and carry it out exactly. if you hit something the plan doesn't cover — a missing file, a failing test, a contradiction between steps — stop and write what blocked you to the log rather than guessing. when done, write a completion summary to /tmp/kimi-delegation-tmp/<task>/result.md: files touched, commands run, anything that failed or was skipped, with the status as its first line (e.g. 'Status: DONE'). as the very last line of your output, print exactly: KIMI_STATUS: <STATE> where STATE is DONE, BLOCKED - <reason>, or FAILED - <reason>." --add-dir <scoped-dir> --output-format text > /tmp/kimi-delegation-tmp/<task>/run.log 2>&1 &
   ```
   if a reliable session identifier is actually available, log it as the
   first line of `run.log` so chetan can reconnect to this conversation
   remotely later — don't fabricate one if there's no real mechanism for it.
3. **heartbeat, not spam.** poll the log every 10s — a loop (`sleep 10`,
   check new lines, repeat), not one long blocking wait. only post to chat
   when something actually changed: a new phase, a flagged issue, an error,
   or completion.
4. **check the sentinel, then stop if it's not DONE — no retries.** once the
   process exits, `tail -n 1 run.log`:
   - `KIMI_STATUS: DONE` → proceed to verification.
   - `KIMI_STATUS: BLOCKED - <reason>` or `KIMI_STATUS: FAILED - <reason>` →
     kimi stopped itself on purpose; tell chetan the reason.
   - no `KIMI_STATUS` line at all after the process has exited →
     unexpected termination (crash, quota cutoff, killed mid-run); tell
     chetan exactly what happened and what state things were left in.

   in every non-`DONE` case: no auto-retry, no auto-recovery — what happens
   next is chetan's call, every time.
5. **verify independently.** start from kimi's `result.md` as an index, not
   as truth — every claim in it still gets checked against rule 2's list,
   every time, even after a clean `KIMI_STATUS: DONE`.
6. **cleanup.** everything for a run — plan, log, result summary, scratch
   copy, any build artifacts kimi left behind — lives under
   `/tmp/kimi-delegation-tmp/<task>/` because rule 1 keeps `--add-dir`
   inside it. once verification's done, ask whether to delete that
   directory or keep it. never delete unasked.

## if the task needs full disk access (rare)

kimi has no FDA of its own; only `/usr/local/bin/bun` does. if a plan
genuinely needs this, treat it as last resort: show chetan the generated bun
script and plist before running `launchctl bootstrap`; unique, greppable
launchd label with a hard timeout and auto-kill; after — always
`launchctl list | grep <prefix>` must return nothing, verify, don't assume
kimi's own cleanup ran.

## when not to use this

schema migrations against real user data, or anything touching
auth/payment/cloud-backup code paths — claude handles those directly instead
of delegating blind.
