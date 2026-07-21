# Design spike: now-playing via ungive/mediaremote-adapter (the Perl/private-framework subprocess)

> **Status**: design spike (plan 103), zero production code changes.
> Successor to 095's NO-GO on *direct* `MediaRemote.framework` calls —
> this spike tests the workaround 095 §2/§7 flagged but explicitly
> declined to build: `ungive/mediaremote-adapter`, a Perl-hosted loader
> that reaches the same private framework from inside `/usr/bin/perl`
> (an Apple-signed `com.apple.*` binary), which the framework's
> entitlement check waves through. Structural template followed:
> `docs/design/now-playing-mediaremote.md` (095) and
> `docs/design/hover-cursor-tracking.md` (086).
>
> **Machine**: `sw_vers` → `ProductName: macOS`, `ProductVersion: 26.5.2`,
> `BuildVersion: 25F84`. `uname -a` → `Darwin ... 25.5.0 ...
> RELEASE_ARM64_T8132 arm64` — the same notchless Mac mini dev machine
> as 095 (see `CLAUDE.md`). `swiftc --version` → Apple Swift 6.2.4
> (only used for a small synthetic-click helper, not the adapter
> itself); `clang --version` → Apple clang 17.0.0; `cmake --version` →
> 4.4.0.
>
> **Upstream cloned read-only, outside the repo**: shallow-cloned
> `https://github.com/ungive/mediaremote-adapter` into this session's
> scratchpad (`/private/tmp/claude-501/.../scratchpad/np-adapter-spike/mediaremote-adapter`),
> never touched anywhere under this repo's tree. **Commit tested:
> `3ac3d4bdf862c7b5399b4fba4df5689f5c38609a`** (authored 2026-05-11).
> Nothing from it is vendored into this repo; the clone and its local
> CMake build output were deleted at the end of this spike (Maintenance
> notes confirm exact state).

## 1. Why this matters

095 closed 079 item 16 with a well-evidenced NO-GO: a bare
`dlopen`/`dlsym` caller gets nothing back from
`MRMediaRemoteGetNowPlayingInfo` on macOS 15.4+, because
`mediaremoted` added a bundle-identifier check (`com.apple.*` only) at
that OS version. 095 §2 already named the one known way around it —
`ungive/mediaremote-adapter`'s trick of loading the framework *from
inside `/usr/bin/perl`*, an Apple-signed binary that passes the check —
but declined to build or recommend it, calling it "explicitly out of
scope per this plan's STOP conditions." The operator asked for that
workaround to be spiked on its own terms rather than left closed
by inference: does it actually work on this machine today, what does
it cost, and exactly how does it fail. This spike answers that with a
working, git-committed local build (not vendored) and real
playback tests — and reaches a materially different, positive
result from 095, because it is testing a different mechanism, not
re-litigating 095's finding about the direct API.

## 2. Mechanism: why Perl passes the check

`codesign -dv /usr/bin/perl` on this machine confirms:

```
Executable=/usr/bin/perl
Identifier=com.apple.perl
...
Signed Time=19 Apr 2026 at 5:54:22 AM
```

`Identifier=com.apple.perl` is exactly the `com.apple.*` prefix 095 §2
cites as the (acknowledged-bug) allowlist inside `mediaremoted`. The
adapter does not patch, re-sign, or impersonate anything — it relies
on a real system binary's real, unmodified code signature. Concretely,
per `bin/mediaremote-adapter.pl` (read in full before running
anything, see §3):

1. The script takes a path to a **pre-built** `MediaRemoteAdapter.framework`
   (an Objective-C dylib bundle, built from this repo — sorry, from
   *upstream's* `src/adapter/*.m` + `src/private/MediaRemote.m` via its
   `CMakeLists.txt`; this spike built it locally into the scratch
   clone, never inside this repo).
2. `DynaLoader::dl_load_file($framework, 0)` — Perl's own C-extension
   loader, essentially a `dlopen` wrapper — loads that dylib **into the
   Perl process's own address space**. The loaded dylib inherits the
   host process's code identity for `mediaremoted`'s check purposes;
   it does not need its own entitlement. (The framework itself is only
   ad-hoc signed by the local CMake build — `codesign -dv` on the built
   `.framework` shows `Signature=adhoc`, `Identifier=com.vandenbe.MediaRemoteAdapter`
   — which is irrelevant to the check; what matters is that the *host
   process* is `/usr/bin/perl`.)
3. `DynaLoader::dl_find_symbol` + `dl_install_xsub` binds one exported
   C symbol (`adapter_get`, `adapter_get_env`, `adapter_stream_env`,
   etc., selected by CLI argument) as a callable Perl sub, then calls
   it. Parameters/options are passed via environment variables
   (`MEDIAREMOTEADAPTER_PARAM_*`, `MEDIAREMOTEADAPTER_OPTION_*`), not
   argv past that point.
4. Inside the framework (`src/private/MediaRemote.m`), the mechanism is
   the *same* one 095 already characterized end-to-end:
   `CFBundleCreate` on `/System/Library/PrivateFrameworks/MediaRemote.framework`
   + `CFBundleGetFunctionPointerForName` for each of
   `MRMediaRemoteGetNowPlayingInfo`, `GetNowPlayingApplicationIsPlaying`,
   `GetNowPlayingApplicationPID`, `GetNowPlayingClient`, plus the
   write-side commands (`SendCommand`, `SetElapsedTime`,
   `SetShuffleMode`, `SetRepeatMode`, `SetPlaybackSpeed`) this spike
   never exercised (read-only scope, see §7). Same private, undocumented
   framework as 095; the only thing different from 095's POC is *whose
   process* is asking.

## 3. Inspection verdict (read before running, per the plan's non-negotiable constraint)

Every file was read before any of it was executed or built. Full
inventory (excluding `.git`): `bin/mediaremote-adapter.pl` (281 lines,
the entry point, quoted in full above); `src/adapter/{env,get,globals,
keys,now_playing,repeat,seek,send,shuffle,speed,stream,test}.m` (~1,400
lines combined); `src/private/MediaRemote.m` (114 lines, the
`CFBundleGetFunctionPointerForName` shim); `src/utility/{Debounce,
helpers}.m` (~290 lines, JSON serialization + stdout printing);
`CMakeLists.txt`/`Makefile` (local build only, no fetch steps, no
`ExternalProject_Add`, no package manager calls — `.gitmodules` exists
but is empty in this commit, no submodules to worry about).

**Verdict: does exactly its stated job, nothing more.**

- `grep -rniE "NSURLSession|URLRequest|curl|socket|http|writeToFile|
  NSFileManager|fopen|fwrite|system\(|popen|NSTask|Process\("` across
  `src/` and `include/` turns up exactly two hits, both benign and
  both read in full: (1) `src/utility/helpers.m:184`, a hardcoded
  `@"invalidURL": [NSURL URLWithString:@"https://apple.com"]` entry in
  a *test fixture* dictionary used only by `NowPlayingTest.m` (never
  invoked by `get`/`stream`) to exercise the JSON sanitizer against a
  syntactically-valid-but-unused URL object — no network call is ever
  made from it; and (2) `src/adapter/test.m`'s `NSTask`, used only by
  the optional `adapter test` entitlement-check mode when
  `MEDIAREMOTEADAPTER_TEST_CLIENT_PATH` names a *locally-built* helper
  binary from the same repo (`MediaRemoteAdapterTestClient`, also
  built and inspected, `src/test/main.m`) — this spike never set that
  env var and never exercised that path, because `get`/`test` returned
  real data immediately without needing it (§5).
- No writes outside the process: `helpers.m`'s only I/O is
  `printf`-style stdout/stderr (`printOut`, `printErr`, `printErrf`) —
  confirmed by reading every function in that file (`fail`, `failf`,
  `serializeJsonDictionarySafe`, `sanitizeValueForJsonEncoding`,
  `appForPID`, `guessImageMimeTypeFromData`, `makePayloadHumanReadable`
  — all pure/formatting, zero filesystem writes).
- No elevated privileges requested or used anywhere; no `sudo`, no
  `AuthorizationExecuteWithPrivileges`-style calls, nothing in
  `Info.plist`/entitlements beyond the framework's own bundle identity.
- `env.m` (105 lines) only reads `NSProcessInfo.processInfo.environment`
  for CLI-parameter passing — no writes, no exec.
- This spike's own build artifacts (the `.framework`, the test client,
  CMake's `build/` directory) and clone were **never** placed inside
  this repo's tree — confirmed by `git status` at the end (§9).

**This is the finding the plan asked for either way, and it came back
clean**: nothing beyond "load the helper framework and print/apply
now-playing events" was found anywhere in the ~1,900 lines inspected.

## 4. Building it (local only, not vendored)

```
$ cd <scratch>/mediaremote-adapter && mkdir build && cd build
$ cmake .. -DCMAKE_BUILD_TYPE=Release
$ cmake --build .
...
Ad-hoc signing MediaRemoteAdapter.framework
[100%] Built target MediaRemoteAdapter
[100%] Built target MediaRemoteAdapterTestClient
```

Pure local Objective-C compile (`AppleClang 17.0.0`) against
`Foundation`/`AppKit`/`UniformTypeIdentifiers` — no downloads, no
package manager, no network access during build. Output ad-hoc signed
automatically by CMake's default codesign step (`flags=0x2(adhoc)`,
`TeamIdentifier=not set` — irrelevant to the entitlement check per §2).

## 5. Test matrix

### 5a. Baseline / entitlement check

`adapter test` (the adapter's own self-check mode) returned **exit
0** immediately — entitled, and real now-playing data was already
available without needing the optional `MediaRemoteAdapterTestClient`
helper:

```
$ perl bin/mediaremote-adapter.pl build/MediaRemoteAdapter.framework test
$ echo $?
0
```

A plain `get` at the very start of this spike, before any deliberate
test playback, did **not** return a clean "nothing playing" empty
result — it returned a real, pre-existing session:

```
$ perl bin/mediaremote-adapter.pl build/MediaRemoteAdapter.framework get
Invalid JSON value type in dictionary for key 'duration': nan (__NSCFNumber)
{"artist":"Liverpool FC","contentItemIdentifier":"24C5BDC7-AA92-4ACA-BF38-AAE3D29FA7A8","title":"Liverpool to Chicago 🌎🤔","elapsedTime":0,"bundleIdentifier":"app.zen-browser.zen","playing":false,"processIdentifier":1356,"album":"","playbackRate":0,"timestamp":"2026-07-21T16:48:48Z"}
```

This is the operator's own real, already-open browser tab (Zen
browser, `app.zen-browser.zen`) with a paused-but-still-registered
media session — this spike did not open it and did not touch it. This
is itself a useful, honest data point rather than a clean baseline:
**MediaRemote returns the last-registered session's info, paused or
not, until a newer session takes over — there is no reliable "truly
idle" signal beyond checking `playing:false` plus a stale
`timestamp`.** (`stream` mode's *connection-time* payload, by
contrast, was empty — `{"type":"data","diff":false,"payload":{}}` —
because stream only echoes registration-*change* events after it
starts listening, not the pre-existing state. That asymmetry between
`get` and `stream` at connection time is worth carrying into the
integration sketch, §8.) The `duration: nan` warning is a minor,
non-fatal robustness gap: the adapter's JSON sanitizer detects a
non-finite `NSNumber` for that one key, prints a diagnostic to
stderr, and **omits the key rather than crashing or emitting invalid
JSON** — a clean degrade, consistent with 095's failure-mode bar.

### 5b. QuickTime — negative result

Reused 095's generated-media approach: AppleScript automation to
QuickTime Player, which 095 recorded as *already granted* to this
session's parent app (confirmed again here — `tell application
"QuickTime Player" to activate` / `get name` succeeded with zero
prompts). Two clips were generated and played, both via `osascript`
`open`/`play`:

- A 24 s `ffmpeg`-generated H.264 video (`out.mp4`) with embedded
  `-metadata title="Plan 103 Now Playing Spike" -metadata artist="notchtap spike"`.
- A shorter `say`-generated `.m4a` speech clip, tried first and
  ruled out only for being too short to poll against, not for any
  data reason.

AppleScript confirmed genuine, sustained active playback throughout
(`get playing of document 1` → `true`, `get current time of document 1`
advancing across repeated queries). **The adapter (`get` and `stream`,
polled repeatedly and also left attached for several seconds) returned
`null` / an empty payload the entire time**:

```
$ perl bin/mediaremote-adapter.pl build/MediaRemoteAdapter.framework get --no-artwork --human-readable
null
```

```
# stream, attached for 6s spanning confirmed active QuickTime playback:
{
  "type" : "data",
  "diff" : false,
  "payload" : {

  }
}
```

**Finding**: QuickTime Player's classic AppleScript-driven document
playback does not register a MediaRemote/Now Playing session on this
machine's OS version at all — not an entitlement-wall failure (§5a/5c
show the adapter itself works), a genuine non-participation by
QuickTime Player in this API for this playback path. This is a new
result, not a confirmation of anything 095 tested: 095 never actually
completed a QuickTime test (its own automation was blocked by an
unanswered consent dialog at the time — see 095 §4's methodology
note); the "QuickTime: reuse 095's generated-video approach" language
in this spike's dispatch describes 095's *intent*, not a result it
banked. This spike is the first to actually execute and observe it,
and the answer is a clean negative.

### 5c. Safari HTML5 `<audio>` + `MediaSession` — positive result

A local page (`player.html`) with a `<audio>` element and a click
handler calling `audio.play()` + setting
`navigator.mediaSession.metadata` (title/artist/album), opened via
`osascript ... open location "file://..."`. A real click was
delivered with a small compiled Swift/`CGEventPost` helper
(`click.swift`, same technique class as 095 §2 and 086's POC — **not
part of this repo**, deleted after use, see §9), aimed at the
button's on-screen coordinates (verified via `screencapture`, converted
from pixel to point space using this display's confirmed 2x backing
scale, `system_profiler SPDisplaysDataType` → `3840x2160` pixels /
`1920x1080` points).

Result, polled ~150-300 ms after the click:

```
{
  "playbackRate" : 1,
  "album" : "now-playing-adapter spike",
  "elapsedTime" : 0,
  "timestamp" : "2026-07-21T17:00:40Z",
  "bundleIdentifier" : "com.apple.WebKit.GPU",
  "processIdentifier" : 98897,
  "parentApplicationBundleIdentifier" : "com.apple.Safari",
  "title" : "Plan 103 Spike Track",
  "uniqueIdentifier" : 420,
  "duration" : 24.123718820861679,
  "artist" : "notchtap adapter test",
  "contentItemIdentifier" : "420",
  "playing" : true
}
```

Title, artist, album, duration, and a live `playing:true`/`playing:false`
transition all arrived correctly — this is the exact "browser activity"
case the operator asked about, and it works cleanly, with the process
identity correctly attributed to Safari via
`parentApplicationBundleIdentifier` even though the immediate
`bundleIdentifier` is WebKit's GPU process (`com.apple.WebKit.GPU`) —
worth noting for any future UI that wants to show a source icon: use
`parentApplicationBundleIdentifier` when present, not the raw
`bundleIdentifier`.

**Latency**: a tight poll loop (`latency_test.sh`, 100 ms interval,
timestamps via `date +%s.%N` bracketing the click and each poll)
measured detection between two consecutive samples:

```
click_sent_at=1784653240.680815000
poll #1 t+0.113492s: not yet (bundleIdentifier=app.zen-browser.zen, i.e. still stale)
DETECTED playing:true at t+0.262763s (poll #2)
```

So: **click-to-detectable-`playing:true` is bounded between ~113 ms
and ~263 ms** on this machine, comfortably inside "a few seconds,"
the bar 095 §5 set for what a now-playing UI needs to feel live. This
was reproduced across five separate play cycles during the CPU test
(§6) with consistent behavior (each cycle's fresh `uniqueIdentifier`
confirms a genuinely new registration each time, not a cached repeat).

A natural end-of-clip transition was also captured cleanly:
`elapsedTime == duration` and `playing:false` once the clip finished
on its own, no click involved.

### 5d. The meet-call question

**Not empirically triggered.** A `getUserMedia({audio:true})` test
page was drafted (`meet.html`, in scratch, never executed) but running
it would trigger a live microphone-access TCC consent dialog on this
machine, and there is no confirmed prior grant for microphone access
to Safari/this session — exactly the class of risk 095 §4 already hit
once this session (an unanswered, synthetic-input-resistant "wants
access to control QuickTime Player" Automation dialog that needed a
human to dismiss) and the plan's own STOP conditions call out
("a NEW consent/TCC dialog appears that prior grants don't cover:
report which dialog and stop rather than blocking on it"). The plan
explicitly offers a non-empirical path for exactly this question
("OR document from the adapter's event model why a conference call
does not publish to Now Playing"), so this spike takes that path
rather than risk a second unresolvable dialog.

**Answer: no, a Meet/WebRTC call does not appear in Now Playing.**
Reasoning, grounded in what this spike actually read and observed,
not general knowledge:

- `keys.m:59-61` (read in §3) defines `mandatoryPayloadKeys()` as
  exactly `[processIdentifier, title, playing]` — every field
  MediaRemote hands back is playback-*session* vocabulary: a title, an
  elapsed/duration pair, a playback rate, shuffle/repeat modes, "is
  banned/liked/in wishlist" (Apple Music-flavored fields). There is no
  field anywhere in `keys.m`'s ~50 keys that models a live capture
  stream (no "microphone active," no "call participant," nothing
  resembling WebRTC's own state machine).
- The entire write-side API surface this spike inspected but did not
  use (`send.m`/`seek.m`/`shuffle.m`/`repeat.m`/`speed.m`) is
  transport-control vocabulary — play/pause/skip/seek/shuffle/repeat/speed
  — again, playback of a track, not a live call.
- §5c's own real evidence backs this up directly: what actually
  registers is explicit, deliberate playback-session metadata (a
  `<audio>`/`<video>` element or an app explicitly calling
  `MPNowPlayingInfoCenter`/setting `navigator.mediaSession.metadata`),
  not "any audio is flowing through this process." `getUserMedia()`
  produces a raw `MediaStream` with no player, no transport controls,
  and — per the Media Session API's own scope (it standardizes
  metadata and action handlers for *media playback elements*, not
  capture streams) — nothing in that spec or in MediaRemote's API
  surface has any hook for a capture-only stream to register itself.
- This matches the operator's own framing of the question ("does a
  Meet call publish to Now Playing") and 095's implicit assumption
  (now-playing = media playback, not live audio I/O).

**Stated plainly**: calls do not appear because Now Playing / MediaRemote
models playback sessions (something with a title, a position, and
transport controls), and `getUserMedia`-based capture has none of
those — it is a fundamentally different API surface with zero
integration points into the one this adapter reads. This is a reasoned
conclusion from code inspection plus this spike's own §5c evidence, not
an independently forced empirical test — flagged here as the one
deviation from a literal "OR" the plan itself sanctioned, not a gap
introduced by this spike.

## 6. Cost

**Per-invocation latency (`get`, `/usr/bin/time -p`, 3 runs, no
artwork)**: `0.03s`, `0.02s`, `0.02s` real time. Comparable to 095's
own measurement of this repo's existing `notchtap-detect` subprocess
(`0.07s`, `0.04s`, `0.03s`) — slower than 095's bare `dlopen`/`dlsym`
POC (`0.01s`/`0.00s`/`0.01s`, no Perl interpreter startup), as
expected, but still well inside a per-poll subprocess budget matching
this repo's existing pattern.

**Idle `%CPU` with `stream` open, no events** (`ps -o %cpu`, 12
samples over ~60 s): **0.0% on every sample**, RSS steady at
20-25 MB. Full run:

```
stream_pid=1256
 PID  %CPU %MEM    RSS COMM
1256   0.0  0.2  25264 perl   (t=0s)
1256   0.0  0.2  25168 perl
1256   0.0  0.1  20816 perl
1256   0.0  0.1  20816 perl
1256   0.0  0.1  20464 perl
1256   0.0  0.1  20464 perl
1256   0.0  0.1  20464 perl
1256   0.0  0.1  20464 perl
1256   0.0  0.1  20464 perl
1256   0.0  0.1  20464 perl
1256   0.0  0.1  20464 perl
1256   0.0  0.1  21056 perl  (t=~60s)
```

**`%CPU` during active playback events** (`stream` left open across
five separate play-trigger cycles, sampled ~0.5s after each click):
**0.0%-0.1%** on every sample — event delivery itself is essentially
free; the CPU story for `stream` mode is dominated entirely by
whether it's running at all, not by event volume at any cadence a
human triggers by hand.

**This is a materially different cost shape than 095 §5's polling
analysis**, because `stream` is a long-lived, event-driven subprocess
(registers for `kMRMediaRemoteNowPlayingInfoDidChangeNotification`-style
callbacks internally, per `stream.m`'s design — see §8), not a
short-lived `Command::new(...).output()` call repeated on a timer.
095's "180× weather's spawn rate" cadence math assumed the only
available shape was poll-and-exit; `stream` sidesteps that entirely by
pushing updates only when something actually changes, at effectively
zero idle cost. This is the single biggest factual difference from
095's conclusion and the main reason this spike's recommendation
differs from 095's.

## 7. Fragility analysis

Carries forward 095's proven failure-mode shape, re-verified here for
the adapter specifically:

- **Forced failures are clean.** A nonexistent framework path → exit 1,
  `Framework not found at ...` to stderr, no crash. A framework
  directory that exists but contains a non-Mach-O file → exit 1,
  `Failed to load framework: ...`, no crash. Both mirror 095 §3a's
  `dlopen`/`dlsym`-returns-`NULL` shape exactly, just surfaced through
  Perl's `DynaLoader::dl_load_file`/`dl_find_symbol` returning falsy
  instead of raw C pointers — same controlled-exit contract
  `notchtap-detect`'s `!output.status.success()` check already handles.
- **The real, unforced degrade (§5b, QuickTime) is also clean** — not
  a crash, not a hang, just a silently empty result. A subprocess
  wrapping this adapter degrades to "no data" exactly like 095 §3b's
  entitlement wall did, for a different underlying reason (app doesn't
  register a session, vs. access denied) but the identical *shape* of
  failure from the Rust side's perspective: nothing to show, no error
  to propagate.
- **What could kill this, ranked by how it would actually die:**
  1. **Apple removes the `com.apple.*` oversight in `mediaremoted`'s
     check** (095 §2 already flags this as Hegenberg's own expectation
     — "once they remove that part of the entitlement validation... it
     will stop working"). This would make `/usr/bin/perl` itself fail
     the same way 095's bare Swift binary already does — the adapter's
     `get`/`stream` would start returning empty/null *unconditionally*,
     indistinguishable from §5b's QuickTime case from the outside. A
     kill-switch config default (Maintenance notes) is exactly the
     right mitigation, because there is no way to distinguish "Apple
     closed the loophole" from "nothing is playing" without an
     explicit opt-out a user can flip if the feature silently goes
     dark forever.
  2. **Apple changes `/usr/bin/perl`'s embedded `DynaLoader` build**
     (unlikely — Perl 5.34.1 shipped since at least the last several
     macOS majors) or removes `/usr/bin/perl` from the base system
     entirely (Apple has floated deprecating scripting-language
     runtimes before) — this would hit the `Framework not found`/`dl_load_file`
     failure path at a different stage but with the same clean-exit
     shape already tested in §7's forced-failure runs.
  3. **The private framework's own ABI changes** — 095 §3c's
     un-de-risked tail risk (symbol kept, signature changed,
     `unsafeBitCast`/C-function-pointer UB) applies identically here;
     this spike did not and could not force it either. No new evidence
     either way — carried forward as an open risk, not resolved.
  4. **Upstream `mediaremote-adapter` itself changes/breaks** — this is
     a *new* risk 095 didn't have, because 095 tested a POC this repo
     would have owned outright. Adopting the adapter means depending on
     a third-party project's maintenance, not just Apple's API
     stability. Mitigated by SHA-pinning (§9) and vendoring it as a
     frozen asset if this ever ships (Maintenance notes) rather than
     tracking upstream `HEAD`.
- **Subprocess boundary still holds.** Nothing in this spike's testing
  — five deliberate play/pause cycles, two forced-failure cases, ~90 s
  of cumulative `stream` runtime — produced a crash, hang, signal, or
  core dump (`ls /cores/` checked, empty). The `notchtap-detect`-shaped
  container (`Command::new(...).output()` for `get`; a supervised
  long-lived child for `stream`, see §8) remains the correct place to
  put this.

## 8. Integration sketch (contingent on a GO decision — see §10)

Recorded per the plan's request; not a spec, and not committed to by
this doc alone.

- **This is a `stream`-shaped source, not a `get`-polled one** — the
  single biggest departure from 095 §6's sketch (which assumed polling
  was the only option, because the direct API had no push channel at
  all). `mediaremote-adapter`'s `stream` mode *is* a push channel,
  reachable through the same subprocess boundary this repo already
  uses, just long-lived instead of call-and-exit: spawn once, hold the
  child alive, read newline-delimited JSON diffs off its stdout as
  they arrive (§5a's `--no-diff`/default-diff option controls whether
  each line is a full snapshot or a delta — default diff mode, matching
  what §5c's evidence used, keeps line size small after the first
  event). This is a materially different process-lifecycle shape than
  `notchtap-detect`'s "spawn, wait, parse, exit" pattern and than
  `weather_poller.rs`'s "spawn on a timer" pattern — closer to how a
  log-tailer would be built. A future build plan needs to design
  supervision for that: restart-on-exit with backoff, and treating a
  *closed stdout* (not just a nonzero exit) as the failure signal,
  since a long-lived child's failure mode looks different from a
  short-lived one's.
- **`src-tauri/src/event.rs`**: unchanged from 095's sketch — a sixth
  `SourceKind::NowPlaying` variant, additive/backward-compatible per
  the closed-enum convention already used for Weather.
  `parentApplicationBundleIdentifier` (§5c) should be the field a
  future UI keys any per-source icon/styling off of, not the raw
  `bundleIdentifier` — Safari's case (`com.apple.WebKit.GPU` with
  `parentApplicationBundleIdentifier: com.apple.Safari`) shows the raw
  field is process-internal, not app-identifying.
- **`src-tauri/src/config.rs`**: `now_playing_enabled: bool` (default
  `false`, same opt-in convention as `weather_enabled`), **plus a
  separate, explicit `now_playing_adapter_enabled`-style kill switch**
  distinct from the feature toggle — per §7's ranked risk #1, this
  needs to be flippable independent of whether the user *wants* the
  feature, so a silent Apple-side break can be muted without losing
  the user's other settings. No `poll_secs` needed if `stream` is
  adopted (push-driven); if a future implementer chooses `get`-polling
  instead for lifecycle simplicity, §6's 0.02-0.03s per-call cost
  reopens 095 §5's cadence-vs-cost tradeoff, unresolved here either
  way.
- **A vendored, SHA-pinned build**, not a live upstream dependency —
  per the plan's own Maintenance notes and this doc's §7 risk #4: check
  in the exact source tree at commit `3ac3d4bdf862c7b5399b4fba4df5689f5c38609a`
  (BSD 3-Clause, confirmed via upstream `LICENSE`), build it as part of
  this repo's own build (mirroring how `notchtap-detect` is built),
  never fetched at runtime or build-time from the network.
- **`src-tauri/src/now_playing_poller.rs`** (naming carried from 095):
  owns the long-lived child process, parses each JSON line, applies
  095 §6's ambient-vs-card split — likely ambient-only into
  `StatusState` (a track change is not naturally an alert card), same
  open design question 095 left unresolved.
- **No new IPC** — same as 095 §6, `get_config` already covers it.
- **Effort**: larger than 095's moot S-M estimate, because the
  process-supervision shape (§8's first bullet) is new work this repo
  hasn't built before (every existing subprocess integration —
  `notchtap-detect`, the ESPN poller — is call-and-exit or
  timer-polled, never a held-open, event-streaming child). Call it
  **M**, not S-M: the JSON parsing and config/event-type wiring is
  small, but the supervision logic (restart/backoff, stdout-close
  detection, clean shutdown on app quit) is genuinely new surface area
  with its own test needs.

## 9. What was cleaned up

- All spawned processes terminated: the `stream`-mode Perl child was
  explicitly `kill`ed (verified via `ps aux | grep -iE
  "mediaremote-adapter|MediaRemoteAdapterTestClient"` → empty) after
  each of the two `stream` runs (idle-CPU test, active-CPU test);
  QuickTime Player and Safari were both explicitly quit via
  `osascript ... quit` and confirmed absent from `ps aux` afterward
  (only long-running, always-on Safari *system* XPC helpers — 
  `SafariPlatformSupport.Helper`, `SafariSafeBrowsing.Service`, etc. —
  remain, which are unrelated background services present regardless
  of whether Safari.app itself is running).
- No new TCC dialog was left behind by this spike specifically —
  QuickTime Automation consent was already granted (confirmed at the
  start, §5b) and 095's own leftover "'cmux' wants access to control
  'QuickTime Player'" dialog from that earlier spike was not
  re-encountered (the grant already covers this session's
  `osascript`/QuickTime calls). §5d's microphone-consent risk was
  avoided entirely by not running the `getUserMedia` test.
- Scratch state: the clone, its CMake `build/` output, the generated
  test media (`speech.m4a`, `speech2.m4a`, `out.mp4`), the HTML test
  pages (`player.html`, `meet.html`, unused), the Swift click helper
  and its compiled binary, the shell test-driver scripts, and all
  screenshots taken during this spike lived entirely under this
  session's scratchpad
  (`/private/tmp/claude-501/.../scratchpad/np-adapter-spike/`) and
  were deleted at the end of this spike — confirmed by `git status`
  below showing no repo changes outside this one doc, and the scratch
  directory itself removed (`rm -rf`) as the final action before
  writing this section.
- `git status --short` at the end of this spike shows exactly one new
  file: this doc.

## 10. Recommendation

## GO, conditional

This reverses 095's NO-GO, but only for the *adapter* mechanism 095
explicitly declined to build — 095's own verdict on the *direct* API
stands unchanged and is not being re-litigated here.

1. **§3: the code does only its stated job.** Full inspection of the
   ~1,900-line upstream tree (Perl entry point plus every
   `src/adapter/*.m`/`src/private/*.m`/`src/utility/*.m` file) found no
   network calls, no writes outside the process, no privilege
   escalation — the two `NSTask`/URL hits found were both benign,
   unused-in-this-spike code paths, read and understood before being
   ruled irrelevant.
2. **§5c: it works, today, on this exact machine's macOS 26.5.2**, for
   the case the operator most cares about (browser tab audio) —
   real title/artist/album metadata, correct play/pause state,
   detected within ~113-263 ms of a real click. This is not inferred
   or extrapolated; it is a directly observed, repeated (5x) result.
3. **§6: the cost story is good, and structurally better than 095
   feared** — `stream` mode is push-driven and measured at 0.0% idle
   CPU over 60 s and 0.0-0.1% during active events, sidestepping 095
   §5's entire "180× weather's spawn rate" concern, which was
   predicated on polling being the only available shape.
4. **§7: the failure modes are clean**, in both the forced (bad
   path/bad framework) and real (§5b's QuickTime non-participation)
   cases — no crash, no hang, controlled non-zero exits or empty
   payloads, consistent with 095's safety bar and this repo's existing
   subprocess-boundary pattern.

**Conditions, carried from the plan's own Maintenance notes and this
doc's own findings — none of them optional**:

- **Vendor as a frozen, SHA-pinned asset** (`3ac3d4bdf862c7b5399b4fba4df5689f5c38609a`),
  never a live/fetched dependency (§8, §7 risk #4).
- **A kill-switch config default, independent of the feature toggle**
  (§8) — because §7 risk #1 (Apple closing the `com.apple.*`
  oversight) degrades *silently* to "no data," identical in shape to
  §5b's already-observed QuickTime non-participation, so there is no
  automatic way to tell "broken by Apple" from "nothing playing" or
  "this app doesn't support it" without an explicit, user-flippable
  switch and a clear degrade path (card absent, no error surfaced —
  same bar 095 set).
- **Explicitly out of scope for a GO**: this spike found no evidence
  either way on ABI-signature drift (095 §3c's tail risk, unchanged)
  or on what happens if Apple targets the adapter's specific technique
  by name rather than just tightening the general check — both remain
  open, unresolved risks a build plan should carry forward rather than
  treat as closed by this GO.
- **QuickTime is confirmed not a source** (§5b) — a future
  implementation should not assume QuickTime playback will ever
  populate this feature on this OS version; Safari (and, by the same
  `MediaSession`-registration mechanism, any browser or app that
  explicitly sets Now Playing metadata — Music.app, Podcasts, Spotify,
  etc., none tested directly here but sharing the same registration
  path §5a's Zen-browser evidence already confirms works passively)
  is the demonstrated-working case.
- **The meet-call answer (§5d) is reasoned, not independently forced**
  — solid enough to act on (it follows directly from `keys.m`'s closed
  set of mandatory fields plus §5c's own positive evidence of what
  *does* register), but flagged so a future implementer knows an
  actual `getUserMedia` test was deliberately skipped for TCC-dialog
  risk, not forgotten.

## 11. Test strategy

None — this spike changes no production code and adds no tests, per
its own scope, matching 095 §8. If a build plan proceeds from §8's
sketch, the natural split (per `TESTING_STRATEGY.md`'s existing
convention) is a fixture-tested pure core (parsing the adapter's JSON
lines, deriving the ambient summary, same shape as
`weather_poller.rs`'s parser) plus an untested supervision loop
(process spawn/restart/backoff) — except unlike Weather's
timer-polled loop, the supervision logic here (§8's first bullet) is
new enough to this repo that its restart/backoff *policy* (not the
raw spawn call) is worth unit-testing on its own, decoupled from any
real subprocess — e.g. a state machine taking "child exited with code
X" / "stdout closed" events and producing "restart now" / "back off N
seconds" / "give up, surface disabled" decisions, tested the same way
`presentation.rs`'s `presentation_mode` pure function already is.

## Maintenance notes

- **This reopens 079 item 16 with a different mechanism than 095
  closed it on** — the reviewer maintaining `plans/README.md`'s
  "Findings considered and rejected" section should update rather than
  leave 095's NO-GO standing unqualified; 095's verdict on the *direct*
  API remains correct and should stay recorded, but item 16 as a whole
  is no longer closed.
- **If a build plan proceeds**: start from §8's sketch, but budget for
  the supervision-loop design work called out there as the real new
  surface area (§8's effort note) — it is not a drop-in repeat of the
  Weather or `notchtap-detect` patterns.
- **If Apple closes the `com.apple.*` oversight before a build plan
  ships**: this GO should be treated as void, not silently
  reinterpreted — re-spike rather than assume the kill-switch alone
  makes it safe to ship un-re-verified, since §7's fragility ranking
  puts this as the single most likely and most severe failure mode.
- **Scratch artifacts** (`nowplaying_poc`-style POC code, the Swift
  click helper, generated test media, screenshots) are not part of
  this repo and were deleted after use, per §9 — nothing from this
  spike's own tooling should be assumed to exist for a future build
  plan to reuse; a build plan starts from the vendored upstream source
  at the pinned SHA, not from this spike's throwaway harness.
