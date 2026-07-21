# Design spike: now-playing as a source via a MediaRemote Swift helper

> **Status**: design spike (plan 095), zero production code changes.
> Researched against commit `9b57037` (worktree fast-forwarded onto
> `master` at dispatch time; drift check `git diff --stat
> 3052de4..HEAD -- src-tauri/src/presentation.rs src-tauri/src/event.rs
> src-tauri/src/config.rs notchtap-detect/ docs/ARCHITECTURE.md` is
> empty at this commit — confirmed). All `file:line` citations below
> were read fresh at this commit.
>
> **A throwaway Swift POC was built, compiled, and run** to resolve the
> two empirical questions this spike turns on (§2, §3). It lived outside
> this repo (`/private/tmp/.../scratchpad/np-spike/`, this session's
> scratchpad — `nowplaying_poc.swift`, plus a `player.html`/`speech.m4a`
> harness used to force a real, confirmed-playing state) and was never
> added to the tauri build, any Cargo/Swift-package target, or any
> render path. It has been deleted; nothing from it is part of this
> commit.
>
> **Machine**: `sw_vers` → `ProductName: macOS`, `ProductVersion: 26.5.2`,
> `BuildVersion: 25F84`. `uname -a` → `Darwin ... 25.5.0 ... RELEASE_ARM64_T8132
> arm64` (Apple Silicon, this repo's usual notchless Mac mini dev
> machine — see `CLAUDE.md`). `swiftc --version` → Apple Swift 6.2.4
> (swiftlang-6.2.4.1.4), target `arm64-apple-macosx26.0`.

## 1. Why this matters

079 item 16 proposed now-playing/media metadata as a sixth `SourceKind`
alongside Football/News/Manual/Cmux/Weather, directly inspired by the
"hover-to-expand, now-playing-style layout" reference that shaped the
whole redesign. The operator decided 2026-07-21 to spike it rather than
drop it outright, overriding the advisor's original recommendation to
retire the idea. This is that spike.

macOS has no public API for system-wide now-playing metadata. The known
route is the private, undocumented `MediaRemote.framework`
(`MRMediaRemoteGetNowPlayingInfo` and friends), reached via
`dlopen`/`dlsym` on a system dylib Apple does not document, ship headers
for, or promise to keep stable. The plan framed the central question as
not "can we render a track title" but **"what exactly are we taking on,
and does it degrade safely when Apple changes it?"** This spike answers
that, and in the process surfaces a second, more immediately decisive
fact: Apple has already changed it, on exactly this machine's OS
version, in a way that defeats the feature outright for an unentitled
process — independent of whether it fails safely.

## 2. Does it work today? The empirical test

**Method**: a standalone Swift CLI (no Tauri, no Rust, no app bundle)
`dlopen`s `/System/Library/PrivateFrameworks/MediaRemote.framework/MediaRemote`,
`dlsym`s `MRMediaRemoteGetNowPlayingInfo`, casts it via `unsafeBitCast`
to `@convention(c) (DispatchQueue, @escaping ([String: Any]) -> Void) -> Void`,
calls it on a background `DispatchQueue.global()` (not `.main` — the
caller thread blocks on a `DispatchSemaphore`, so dispatching the
callback to `.main` would deadlock with nothing pumping the run loop),
and prints a JSON summary of the returned dictionary to stdout,
mirroring `notchtap-detect`'s JSON-to-stdout contract.

**Baseline run, nothing deliberately playing:**

```
$ ./nowplaying_poc
stage: dlopen(/System/Library/PrivateFrameworks/MediaRemote.framework/MediaRemote)
stage: dlopen OK, handle=0x000000036f53c7b0
stage: dlsym(MRMediaRemoteGetNowPlayingInfo) OK, sym=0x00000001aa878140
stage: callback invoked, 0 keys
{ "ok": true, "nothing_playing": true }
$ echo $?
0
```

`dlopen` and `dlsym` both succeed immediately, with **no permission
prompt of any kind** — matching plan 086's finding for tracking areas
(§4 below has the full entitlement analysis). The call completes, the
callback fires, and the returned `NSDictionary` is empty (`0 keys`).
That alone is ambiguous — it could mean "nothing is playing" or "access
is silently blocked" — so the load-bearing test is forcing a **known,
confirmed-playing** state and checking again.

**Forcing a real playing state**: Apple Music/Podcasts had no usable
library content on this machine and AppleScript control of Music.app/
QuickTime Player hung on `-1712 (AppleEvent timed out)` — this
machine's "Automation" TCC layer has no prior grant for scripting those
apps, and no interactive human was available to answer the resulting
consent dialog (see §4's methodology note — this is a *testing*
obstacle, unrelated to MediaRemote's own access model, and is called
out separately so it isn't conflated with the actual finding). Instead:
a local HTML page (`player.html`) with an `<audio>` element
(`speech.m4a`, a 26.5 s `afconvert`-transcoded `say` TTS clip) and a
click handler that calls `audio.play()` and sets
`navigator.mediaSession.metadata` (title/artist/album) was opened in
Safari. A synthetic `CGEventPost` click (a small `click.swift` helper,
same class of technique as plan 086's POC) landed on the page's button.
A screenshot taken immediately after confirms genuine playback: the
button's text changed to **"PLAYING"** and Safari's toolbar shows the
tab-audio (speaker) icon next to the reload button — this is real,
user-gesture-backed HTML5 audio playback with `MediaSession` metadata
set, the same mechanism third-party "Now Playing" menu-bar utilities
rely on for browser tabs (e.g. YouTube in Control Center's Now Playing
widget).

**Polling `nowplaying_poc` immediately after, while Safari's own UI
confirmed active playback:**

```
$ ./nowplaying_poc
stage: dlopen(...) OK
stage: dlsym(MRMediaRemoteGetNowPlayingInfo) OK
stage: callback invoked, 0 keys
{ "ok": true, "nothing_playing": true }
```

**Still empty.** This is the central, load-bearing empirical result of
this spike: `MRMediaRemoteGetNowPlayingInfo`, called from a bare,
unbundled, ad-hoc-compiled `swiftc` binary, returns nothing on this
machine's macOS version **even during confirmed-active playback**. This
is not a "nothing is playing" false negative — it is a real access
restriction, and it is a well-documented one (not a novel discovery of
this spike):

- Apple added an entitlement/identity check inside the system
  `mediaremoted` daemon starting **macOS 15.4** (Sequoia). Per
  BetterTouchTool's developer Andreas Hegenberg (a corroborating,
  independent, production-shipping source — [BetterTouchTool community,
  "Now Playing is no longer working on MacOS 15.4"](https://community.folivora.ai/t/now-playing-is-no-longer-working-on-macos-15-4/42802)):
  *"I think the MediaRemote framework now requires special
  entitlements, so it can not be used any longer by third party apps
  ... It only works because Apple forgot to remove the `com.apple`
  check in their logic when they introduced the entitlement
  protection."* Only processes whose bundle identifier begins with
  `com.apple.` pass the check — an acknowledged Apple **oversight**,
  not a documented, requestable capability.
- [`LyricFever` issue #94](https://github.com/aviwad/LyricFever/issues/94)
  reports the same restriction (macOS 15.3/15.4 beta) with a precise
  error: `kMRMediaRemoteFrameworkErrorDomain` code 3, *"Operation not
  permitted."* Notably, `MRMediaRemoteCommand` (playback *control* —
  play/pause/skip) **kept working**; only the *read* path
  (`GetNowPlayingInfo`) was restricted. This is a second, independent
  confirmation that the restriction fails as a clean, typed error, not
  a crash.
- [`nowplaying-cli` issue #28](https://github.com/kirtan-shah/nowplaying-cli/issues/28)
  confirms the same break on macOS 15.4 and lists other affected
  tools: BetterTouchTool, Keyboard Maestro, and — most directly
  relevant to this spike — **`boring.notch`**, a notch-overlay utility
  in the same product category as this app. Its own issue tracker
  ([#417](https://github.com/TheBoredTeam/boring.notch/issues/417),
  [#434](https://github.com/TheBoredTeam/boring.notch/issues/434))
  shows now-playing metadata (album art, slider, visualizer) went
  blank on 15.4 while playback *controls* kept working — the exact
  read/control split `LyricFever` reported.
- This machine's `sw_vers` (26.5.2) is well past the 15.4 threshold, so
  this empirical result is expected, not anomalous.

**What DOES still work, per those same sources**: a small number of
system binaries (e.g. `/usr/bin/perl`, reported bundle id
`com.apple.perl`) still pass the `com.apple.` check, so
[`ungive/mediaremote-adapter`](https://github.com/ungive/mediaremote-adapter)
loads the framework *from inside Perl* and shells out to it, and
`boring.notch` now ships that adapter to keep the feature working.
Hegenberg's own assessment of that class of workaround: *"Once they
remove that part of the entitlement validation for the MediaRemote
framework, it will stop working."* — i.e. even the working fix is
understood by its own downstream adopters as exploiting an
acknowledged bug in Apple's check, not a stable interface. This spike
does not build or recommend that workaround — see §3 and §7.

## 3. How does it fail? (the load-bearing question)

Two axes matter here, and they point in different directions.

### 3a. Forced low-level failures — clean, as designed

```
$ ./nowplaying_poc --bad-path
stage: dlopen(/System/Library/PrivateFrameworks/DoesNotExist.framework/DoesNotExist)
PROCESS: dlopen returned NULL, no crash, no signal, continuing normally to controlled exit
{ "ok": false, "stage": "dlopen", "detail": "dlopen(...): tried: '/System/Library/PrivateFrameworks/DoesNotExist.framework/DoesNotExist' (no such file), ... (no such file, not in dyld cache)" }
$ echo $?
2

$ ./nowplaying_poc --bad-symbol
stage: dlopen(/System/Library/PrivateFrameworks/MediaRemote.framework/MediaRemote)
stage: dlopen OK, handle=0x000000036f53c7b0
PROCESS: dlsym returned NULL, no crash, no signal, continuing normally to controlled exit
{ "ok": false, "stage": "dlsym", "detail": "dlsym(0x36f53c7b0, MRThisSymbolDoesNotExist12345): symbol not found" }
$ echo $?
3
```

Both forced failures — a bogus dylib path (simulating "the framework
moved/was removed") and a bogus symbol name (simulating "the symbol was
renamed") — resolve to a **clean, controlled exit**: `dlopen`/`dlsym`
return `NULL`, the process prints a diagnostic and exits with a
deliberate non-zero code (2/3), no signal, no crash, no core dump
(`ls /cores/` empty before and after). This is exactly the shape
`notchtap-detect` is built around (`presentation.rs:82-101`:
`Command::new(detect_path).output()`, `!output.status.success()` →
`anyhow` error) — a non-zero exit is all the Rust side needs to fall
back cleanly, the same pattern `detect_mode`'s `nonzero_exit_falls_back_to_hud`
test already asserts for `notchtap-detect` (`presentation.rs:193-200`).

### 3b. The real, unforced failure — also clean, but disqualifying

§2's entitlement-wall result is itself a failure mode, and it is
important that it is the **same shape** as the forced ones: no crash,
no hang, no garbage — an empty dictionary (this spike's POC) or a typed
`kMRMediaRemoteFrameworkErrorDomain` error (LyricFever's report, using
a different, error-returning entry point). By the plan's own bar — "a
source that can hard-fail the app is disqualifying; one that degrades
to 'no data' is acceptable" — **this passes the safety bar**. The
problem is not that it fails unsafely; it's that, on any macOS version
this app would plausibly run on today (15.4+, and this machine is far
beyond that), it doesn't work *at all* for an unentitled, unbundled
process, safety aside.

### 3c. What this spike could NOT force or verify

- **ABI/signature drift, as opposed to removal.** §3a tested "the
  symbol is gone" (`dlsym` → `NULL`) and "the framework is gone"
  (`dlopen` → `NULL`) — both are caught by simple pointer-null checks
  before any call happens. A subtler case — Apple keeps the symbol name
  but changes its argument types or count — is not something this
  spike could force without an actual differing OS build to test
  against, and it is categorically different: `unsafeBitCast`ing a
  function pointer to a function type with the wrong ABI is undefined
  behavior in C-calling-convention terms and could plausibly crash
  rather than return cleanly. This spike did not observe a crash in any
  of its real calls (the ABI signature used — matching the
  publicly-documented-by-convention-only shape multiple independent
  OSS projects, e.g. `nowplaying-cli`/`mediaremote-rs`, use — worked
  cleanly, including the "0 keys" empty-dict case, which is itself
  evidence the calling convention is currently correct), but "no crash
  observed in one run on one OS version" is not proof this can never
  crash on a future OS. **This is the one genuinely un-de-risked tail
  risk of the `dlopen`/`dlsym` approach**, and it's exactly why the
  subprocess boundary (§7) matters even in a GO-IF world.
- **Whether `Command::output()` on the Rust side actually survives a
  child-process crash/signal, not just a clean non-zero exit.** This
  spike did not write or run any Rust code (zero production code,
  per scope). Rust's documented `std::process::ExitStatus::success()`
  semantics (Unix: `true` only for a normal `exit(0)`; a
  signal-terminated child reports `success() == false` via `output()`,
  which returns `Ok` regardless of how the child died) support the
  "a crashing subprocess is caught by the same `!output.status.success()`
  check, not a `main.rs` crash" safety argument — but that is
  documented standard-library behavior cited here, **not independently
  re-verified in this spike**, the same honesty gap 086 flagged for
  `CGEventTap`'s Input Monitoring requirement.

## 4. Entitlements, TCC, and notarization

**MediaRemote itself: no user-facing prompt, ever, in this spike.**
Every `dlopen`/`dlsym`/`MRMediaRemoteGetNowPlayingInfo` call — across
the baseline run, the confirmed-playing run, and both forced-failure
runs — completed with zero System Settings prompts, zero
`tccd`-mediated dialogs, and no entry required in Privacy & Security.
This matches 086's tracking-area finding (§4 there): the *mechanism* of
reaching the framework is unrestricted. What's restricted is what the
framework *hands back* once reached (§2/§3b) — a silent, code-level
gate (bundle-identifier prefix check inside `mediaremoted`), not a
grantable permission. There is no "Now Playing access" toggle in
System Settings a user could flip for this app, because the check
being applied is not a TCC service at all — it's bespoke logic inside
Apple's own daemon. Searching Apple's public entitlements catalog
(`developer.apple.com/documentation/bundleresources/entitlements`)
turns up `com.apple.mediaremote.set-playback-state` (a *control*-side,
CarPlay-flavored entitlement, part of the public `MPRemoteCommandCenter`
surface) but nothing documented, requestable, or matching the *read*
restriction found here. There is no known formal path — via Apple's
managed-capability request process or otherwise — to obtain what would
be needed.

**Separately — a methodology note, not a MediaRemote finding**: while
trying to force a real "playing" state for §2, this spike's own testing
setup hit a *different*, unrelated TCC layer: AppleScript "Automation"
consent. `osascript -e 'tell application "Music" to play'` and a
`playing of document 1` query to QuickTime Player both hung on `-1712
(AppleEvent timed out)`, and a screenshot later confirmed why: a real,
unanswered "'cmux' wants access to control 'QuickTime Player'" consent
dialog was sitting on screen, spawned by this session's own `osascript`
calls, that no interactive human was present to click through — and,
notably, that dialog **did not respond to synthetic `CGEventPost`
clicks or `System Events` keystrokes** aimed at it (two independent
attempts, both no-ops), consistent with Apple's known hardening of TCC
consent panels against synthetic-input clickjacking. **This dialog is
still on screen on the dev machine as of this writing** — see the
report's NOTES for exactly what to dismiss. It is unrelated to whether
notchtap could read now-playing info; it only affects this spike's own
attempt to script *playback* for testing purposes, via a completely
different app-automation permission than anything a shipped now-playing
poller would need (a poller would only ever call `dlopen`/`dlsym`, the
same call proven prompt-free above).

**Notarization / distribution**: `docs/ARCHITECTURE.md` §9 already
locks this app's distribution model as local-only (build-on-each-machine
or transfer without the quarantine flag), with the Mac App Store
explicitly ruled out for unrelated reasons (sandboxing blocks the
persistent overlay). Two facts matter for a private-framework
dependency specifically: (1) **notarization is not App Review** — the
notary service scans for malware signatures and validates code-signing/
entitlements hygiene; it does not statically reject private-API usage
the way Mac App Store review's automated+human check does (per Apple
Developer Forums thread 702740 and the general Developer-ID vs.
App-Store distinction — Developer ID + notarization has shipped
private-API-using apps for years; only App Store submission blocks
that class of app). Since §9 already rules out App Store distribution
and doesn't currently require notarization at all, **this is a
deferred risk, not a blocker**, exactly as the plan anticipated. (2) If
this app ever adopted the `mediaremote-adapter`-style workaround
(piggybacking on `/usr/bin/perl`'s trusted bundle identity), that is a
materially different risk category from "uses a private framework" —
it is actively exploiting an acknowledged Apple bug to defeat an access
control, which Hegenberg (its own most prominent production adopter)
expects Apple to eventually close. That durability risk is independent
of notarization and is covered in §7.

## 5. Poll/wake cost

Since §2/§3b already establish the direct API doesn't return usable
data on this machine, this section is bounded/hypothetical — recorded
because the plan asks for it and because it matters if Apple ever
narrows or removes the restriction.

**Measured per-invocation cost** (subprocess spawn + `dlopen` +
`dlsym` + the async callback round-trip, `/usr/bin/time -p`, 3 runs):
`0.01s`, `0.00s`, `0.01s` real time. For comparison, this repo's
already-shipping `notchtap-detect` subprocess (AppKit `NSScreen`
query, same `Command::new(...).output()` invocation shape, called at
minimum at startup) measured **slower**: `0.07s`, `0.04s`, `0.03s`
across 3 runs on the same machine — AppKit's own startup cost
apparently dominates over the bare `dlopen`/`dlsym` path. So a
now-playing subprocess would likely be *cheaper per call* than the
precedent this repo already ships.

**The real cost question is cadence, not per-call price.** This repo
has a documented idle-CPU discipline: plan 015 replaced a 250 ms
heartbeat specifically because sub-second repeating timers defeat App
Nap/timer coalescing (~345,000 wakeups/day eliminated), and plan 018
cut a `background-position`-animated repaint for the same reason. The
one existing *poller* precedent, weather (`weather_poll_secs` default
**900s** — `config.rs`), works because current-conditions weather is
genuinely slow-changing; a 15-minute staleness window is invisible to
a user. **Now-playing is not that kind of source.** A "what's playing"
UI element that updates every 15 minutes reads as broken — users expect
track-change and play/pause-state updates within a few seconds, which
is a fundamentally different, much poll-hungrier cadence than every
other source this app has ever shipped: at a 5 s interval (still
sluggish by media-widget standards) that's ~17,280 subprocess spawns/
day, versus weather's 96/day — roughly **180× weather's spawn rate**
for a source that, per §2/§3b, currently returns nothing anyway.
MediaRemote has **no push/notification channel reachable through the
subprocess boundary** (the framework does expose
`MRMediaRemoteRegisterForNowPlayingNotifications`, a
long-lived-process callback API — not something a short-lived,
`Command::new(...).output()`-invoked, print-and-exit helper can use;
adopting it would mean a persistent subprocess with its own IPC to the
Rust core, a materially different and heavier design than
`notchtap-detect`'s call-and-exit shape). So even setting the
entitlement wall aside, this source is a structurally worse fit for
this repo's polling posture than any source it currently ships.

## 6. What the shipped shape would be (sketch, not a spec)

Recorded per the plan's request, for a future build plan to start from
— **contingent on §2's entitlement wall being resolved**, which it
currently is not (§7).

- **`src-tauri/src/event.rs`**: a sixth `SourceKind::NowPlaying` variant
  (the enum is closed and rejects unknown values at deserialization —
  `event.rs:83-89` — so this is an additive, backward-compatible wire
  change, same shape as Weather's addition).
- **`src-tauri/src/config.rs`**: `now_playing_enabled: bool` (default
  `false`, same opt-in-ambient convention as `weather_enabled`/
  `rss_enabled`), `now_playing_poll_secs: u64` — needs a much smaller
  default than weather's 900s per §5 (a real default would need its
  own trade-off discussion between staleness and spawn rate; this
  spike does not pick one).
- **A new Swift helper**, `notchtap-nowplaying/` (own `Package.swift` +
  `Sources/notchtap-nowplaying/main.swift`), mirroring
  `notchtap-detect`'s exact contract: no arguments, JSON to stdout,
  non-zero exit on any failure — same `dlopen`/`dlsym` logic this
  spike's POC already validated, moved into a proper subprocess
  alongside the existing one rather than replacing it.
- **`src-tauri/src/now_playing_poller.rs`**: mirrors
  `weather_poller.rs`'s ambient-vs-card split (`weather_poller.rs:1-20`)
  — current track/artist/playback-state is **ambient**, folded into
  the idle-rail `StatusState` via a new `engine.update_now_playing`
  (compare-then-store, wake only on change, never an `Event`), the
  same mechanism `update_weather` uses. Unlike weather, there is
  probably no natural "alert card" analog (a track change isn't an
  alert), so this source may end up ambient-only — a real design call
  for whoever writes the build plan, not resolved here.
- **No new IPC**: `get_config`/`get_default_config` already serialize
  the whole `Config`; the settings window would gain a section and,
  per the v5.1 convention, could reuse the existing
  `send_test_notification` per-source test button — no new
  `#[tauri::command]` needed for that part.
- **Coarse effort estimate**: mirrors plan 040 (Weather)'s shape and
  size (a ~700-line poller + a ~25-line Swift shim + config fields +
  tests) — call it **S–M**, comparable to Weather's own effort,
  *if* §2's wall were resolved. As things stand today, this estimate
  is moot: there is nothing to poll.

## 7. Recommendation

## NO-GO

This is a well-evidenced NO-GO, not a hedge. Three independent findings
converge:

1. **§2/§3b: it does not work today.** On this exact machine's macOS
   version (26.5.2, well past the documented 15.4 threshold), a bare
   `dlopen`/`dlsym` caller receives nothing from
   `MRMediaRemoteGetNowPlayingInfo` — verified empirically with a
   confirmed-active playback source, not inferred from the empty
   "nothing playing" baseline alone — and this matches, precisely, a
   platform restriction independently reported by four unrelated
   projects, one of which (`boring.notch`) is in this app's own product
   category and hit the identical read/control split.
2. **There is no legitimate path to the access this app would need.**
   The gate is an unpublished identity check inside `mediaremoted`, not
   a documented, requestable entitlement — Apple's own capability-
   request process has nothing matching it. The only known workaround
   (`mediaremote-adapter`'s `/usr/bin/perl` piggyback) exploits an
   acknowledged Apple bug that its own most prominent adopter expects
   Apple to close, and doing so would mean shipping a technique whose
   premise is "borrow a system binary's trusted identity to defeat an
   access check" — a materially different and shakier posture than
   this app's existing, fully-legitimate `notchtap-detect` subprocess.
   Building that is explicitly out of scope per this plan's STOP
   conditions ("a well-evidenced NO-GO... rather than hunting
   workarounds").
3. **Even setting (1) and (2) aside, §5's cadence math is a bad fit.**
   Now-playing wants near-real-time updates with no push channel
   available through the subprocess boundary, putting it at roughly
   180× weather's spawn rate for meaningfully fresher data — a
   structurally worse match for this repo's documented idle-CPU
   discipline (plans 015/018) than any source shipped so far.

**What DID go right, and is worth preserving as a reusable finding**:
the subprocess-boundary safety argument holds. §3a's forced failures
(bad path, bad symbol) and §3b's real failure (the entitlement wall)
all degrade cleanly — no crash, no hang, no garbage, controlled non-
zero exits with diagnostics. If some *other* private-framework idea
comes up later, the `notchtap-detect`-shaped subprocess pattern remains
the correct container for it: **a private framework that fails takes
down a disposable, `Command::new(...).output()`-invoked subprocess, not
the overlay** — this spike found no evidence against that claim, only
the one open tail-risk in §3c (ABI-signature drift, not exercised here)
that argues for the subprocess boundary rather than against it.

This closes 079 item 16.

## 8. Test strategy

None — this spike changes no production code and adds no tests, per
its own scope. If a future OS change or a legitimate Apple entitlement
route reopens this (see Maintenance notes), a build plan starting from
§6's sketch should follow `weather_poller.rs`'s existing pattern: a
fixture-tested pure core (parsing the helper's JSON, deriving the
ambient summary) plus an untested outer spawn loop, matching
`TESTING_STRATEGY.md`'s stated convention for this shape of component.

## Maintenance notes

- **Do not re-litigate this without new information.** Per this plan's
  own maintenance instructions, this NO-GO should be recorded in
  `plans/README.md`'s "Findings considered and rejected" section (left
  to the reviewer maintaining that index — this spike's dispatch
  explicitly skipped that edit) so 079 item 16 is not re-audited from
  scratch. The two things that would legitimately reopen it: **(a)**
  Apple documents and makes requestable a real read entitlement for
  now-playing info (unlikely but possible — Hegenberg's cited feedback
  ID `FB18112000` requests exactly this), or **(b)** this app's threat
  model changes enough that the `mediaremote-adapter`-style workaround
  becomes acceptable — a call this spike explicitly declines to make
  and flags as a STOP condition, not a judgment call.
- **A real system dialog needs manual dismissal on the dev machine.**
  This spike's own test methodology (§4's methodology note) left an
  unanswered "'cmux' wants access to control 'QuickTime Player'"
  Automation consent dialog on screen — it resisted synthetic dismissal
  (§4) and this spike could not clear it programmatically. Recommend
  clicking **"Don't Allow"** (no legitimate need for that access
  exists). This is inert (grants nothing on its own) but is a real,
  visible side effect of this session's testing that a human should
  clear.
- **The POC and its testing harness are not part of this repo.** Both
  lived in this session's scratchpad (`nowplaying_poc.swift`,
  `click.swift`, `player.html`, `speech.aiff`/`speech.m4a`) and were
  deleted after use; `git status --short` at the end of this spike
  shows only this doc. Apps opened for testing (Safari, Music,
  QuickTime Player) were quit at the end of the session — Safari had
  only this spike's own test window open (verified via `System Events`
  window enumeration before quitting), so no pre-existing tabs/session
  state were at risk.
- **If §2's restriction is ever lifted**, §6's sketch is a reasonable
  starting point for a build plan, but §5's cadence tension (no push
  channel, near-real-time UX expectations) is a real, unresolved design
  question that sketch deliberately does not answer — it would need
  its own decision, not an assumption inherited from weather's 900s
  default.
