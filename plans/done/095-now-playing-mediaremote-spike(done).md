# Plan 095: SPIKE — now-playing as a source via a MediaRemote Swift helper (079 item 16)

> **Executor instructions**: This is a SPIKE, matching the shape of plans
> 030/031/049-053/086 — **the deliverable is a written design doc plus a
> clear go/no-go recommendation, NOT a shipped feature**. Produce
> `docs/design/now-playing-mediaremote.md` and change **zero** production
> code. A throwaway POC outside the build tree is allowed and encouraged
> (plan 086's precedent — its POC lived in a scratchpad, never in the
> repo). When done, update the status row for this plan in
> `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 3052de4..HEAD -- src-tauri/src/presentation.rs src-tauri/src/event.rs src-tauri/src/config.rs notchtap-detect/ docs/ARCHITECTURE.md`
> Expected: empty. This spike reads these as precedent, so a diff means
> re-read before citing.

## Status

- **Priority**: P3
- **Effort**: S–M (investigate)
- **Risk**: LOW for the spike itself (zero production code); the risk it
  exists to *measure* is HIGH — an undocumented private framework.
- **Depends on**: none. Independent of the 091/092/093 chain and of 094.
- **Category**: direction
- **Planned at**: commit `3052de4`, 2026-07-21

## Why this matters

079 item 16 proposed now-playing/media metadata as a genuinely new source
type — a sixth `SourceKind` alongside Football/News/Manual/Cmux/Weather —
directly inspired by the "hover-to-expand, now-playing-style layout"
reference that shaped the whole redesign. The operator **decided on
2026-07-21 to spike it rather than drop it**, explicitly overriding the
advisor's recommendation to retire the idea. This plan is that spike.

The reason it needs a spike and not a build plan: **macOS has no public
API for system-wide now-playing metadata.** The known route is the
private, undocumented `MediaRemote.framework` (`MRMediaRemoteGetNowPlayingInfo`
and friends), obtained via `dlopen`/`dlsym` on a system dylib Apple does
not document, ship headers for, or promise to keep stable. Apple has
tightened this surface repeatedly across releases. So the central
question is not "can we render a track title" — it is **"what exactly are
we taking on, and does it degrade safely when Apple changes it?"**

## The spike's central question (answer this, in this order)

1. **Does it work today, on this machine's macOS version?** Build a
   throwaway Swift CLI that `dlopen`s MediaRemote and prints now-playing
   JSON to stdout. Report the exact macOS version tested and the exact
   symbol-resolution path used.
2. **How does it fail?** The load-bearing question. When the framework is
   absent, the symbols move, or Apple restricts access: does the helper
   exit non-zero cleanly (like `notchtap-detect` does — see Precedent),
   or does it crash / hang / emit garbage? **A source that can hard-fail
   the app is disqualifying; a source that degrades to "no data" is
   acceptable.** Demonstrate the failure mode empirically (e.g. by
   pointing `dlopen` at a bogus path) rather than reasoning about it.
3. **Is there any entitlement, TCC prompt, or sandbox interaction?**
   Compare against 086's finding that tracking areas needed *no*
   permission prompt and no capabilities change. If now-playing triggers
   a user-visible authorization prompt, say so plainly — that changes the
   product decision, not just the engineering one.
4. **What is the polling/wake cost?** This repo has a documented
   idle-CPU history (plans 015/018 cut wake rates deliberately; plan 086
   rejected polling-based hover for exactly this reason). Now-playing has
   no push channel through a subprocess, so state a concrete poll
   interval and its measured cost, or explain why a notification-based
   variant is possible.
5. **What would the shipped shape be, concretely?** A sixth `SourceKind`
   + a poller + config flag mirrors `weather_enabled`'s opt-in pattern.
   Sketch it (files, config keys, wire fields) with a coarse effort
   estimate — enough for a future build plan to be written from the doc,
   not a full spec.
6. **Distribution reality**: does depending on a private framework
   endanger notarization, or any future distribution path recorded in
   `docs/ARCHITECTURE.md`? If distribution is local-only today, say that
   and note it as a deferred risk rather than a blocker.

## Precedent to follow (read these; the recommendation should slot into them)

- **The subprocess boundary is the established pattern.**
  `docs/ARCHITECTURE.md` §5 locks "the swift↔rust boundary is a
  subprocess, not ffi." The working example is `notchtap-detect/`
  (`Package.swift` + `Sources/notchtap-detect/main.swift`), invoked from
  `src-tauri/src/presentation.rs:82-101`: `Command::new(detect_path)
  .output()`, non-zero exit → `anyhow` error, stdout parsed as JSON via
  `parse_detect_output`. Its path is config-driven with a default of
  `/usr/local/bin/notchtap-detect` (`config.rs:187-189`). **A
  now-playing helper should be the same shape** — a second small Swift
  CLI printing JSON, never FFI into the tauri process. Say explicitly
  whether that holds, because it is also the safety argument: a private
  framework that crashes takes down a subprocess, not the overlay.
- **The spike-doc shape** is `docs/design/hover-cursor-tracking.md`
  (plan 086): a status header pinning the researched commit, numbered
  sections, an explicit empirical-test section, security and perf
  analyses, a recommendation, and honest limits ("documented but not
  re-verified", hardware unavailable). Match that structure and that
  candor — 086's value came from labelling what it had *not* proven.
- **The sixth-source shape** would extend `SourceKind`
  (`src-tauri/src/event.rs:83-89`, a closed set whose unknown values are
  rejected at deserialization) and follow `weather_enabled`'s
  default-`false` opt-in convention.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| macOS version | `sw_vers` | record exactly |
| Build the POC | `swiftc <poc>.swift -o /tmp/np-poc` (outside the repo) | compiles |
| Existing shim reference | `cat notchtap-detect/Sources/notchtap-detect/main.swift` | the JSON-to-stdout pattern to mirror |
| Repo stays clean | `git status --short` | only `docs/design/now-playing-mediaremote.md` + `plans/` |

## Scope

**In scope**: `docs/design/now-playing-mediaremote.md` (create) — and
nothing else in the repo.

**Out of scope — all of it, this is a spike**:
- `src-tauri/**`, `src/**`, `notchtap-detect/**` — zero production code.
  Do NOT add a second Swift package to the repo; the POC lives in a
  scratchpad/tmp path and is disposable.
- `event.rs`'s `SourceKind` — sketch the sixth variant in prose only.
- `tauri.conf.json`, `capabilities/`, `build.rs`.
- Any dependency addition anywhere.

## Steps

1. Read the precedent (ARCHITECTURE.md §5, `notchtap-detect`'s
   `main.swift`, `presentation.rs:82-101`) and record the exact
   subprocess contract the helper would have to satisfy.
2. Build and run the throwaway POC outside the repo. Capture real
   stdout for a playing track, a paused track, and nothing playing.
3. **Force the failure modes** (bogus dylib path; symbol not found) and
   record exactly what happens to the process.
4. Investigate entitlements/TCC and notarization implications; cite
   sources rather than asserting.
5. Measure or bound the poll cost; state a concrete interval.
6. Write `docs/design/now-playing-mediaremote.md` in 086's structure,
   ending with an unambiguous **GO / NO-GO / GO-IF** recommendation and
   a coarse build estimate. If the answer is NO-GO, say so — a
   well-evidenced "don't build this" is a successful spike, and the
   advisor's prior recommendation was to drop it.
7. Delete the POC; confirm the repo is clean apart from the doc.

**Verify (each step)**: real command output recorded in the doc; final
`git status --short` shows only the new doc and `plans/` changes.

## Test plan

None — no production code. The empirical POC results ARE the evidence,
and they belong in the doc with the commands that produced them.

## Done criteria

- [ ] `docs/design/now-playing-mediaremote.md` exists, follows 086's
      section structure, and answers all six central questions
- [ ] The failure-mode question (#2) is answered with observed evidence,
      not reasoning
- [ ] An explicit GO / NO-GO / GO-IF recommendation with a coarse effort
      estimate
- [ ] `git status --short` shows no production-code change anywhere
- [ ] The POC is deleted and lives nowhere in the repo
- [ ] `plans/README.md` status row updated

## STOP conditions

- The POC requires adding a dependency to the repo, or cannot be built
  outside the repo tree.
- MediaRemote access turns out to require an entitlement this app cannot
  obtain — that is a finding, not a blocker: write it up and recommend
  NO-GO rather than hunting workarounds.
- You find yourself editing any file under `src-tauri/` or `src/`.
- Any workaround involves injecting into, or scraping the UI of, another
  application — out of bounds; report instead.

## Maintenance notes

- If the verdict is GO or GO-IF, the build becomes a NEW plan (next free
  number) with the doc as its rationale of record — this spike does not
  authorize implementation.
- If NO-GO, record it in `plans/README.md`'s "Findings considered and
  rejected" section so item 16 is not re-litigated, and 079 item 16
  closes permanently.
- Either way, this is the last unfiled item from plan 079's ledger.
