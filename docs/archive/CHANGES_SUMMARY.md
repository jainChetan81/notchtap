# All Changes Made to `mac-notification-nudge` Docs

> Historical record — file paths below reflect the layout at the time of
> this changelog (docs at repo root). The planning docs now live under
> `docs/` (`../ARCHITECTURE.md`, `../IMPLEMENTATION_PLAN.md`,
> `../TESTING_STRATEGY.md`); `CLAUDE.md` and `README.md` are still at the
> repo root. Not updated inline below to preserve the record as-written.

> For the executor who wrote the original plan.  
> Every edit is documented below. Each file is listed with its changes, and the rationale is provided so you know why it was added.

---

## Summary of Decisions Locked

| Decision | Value | Rationale |
|---|---|---|
| **Binary / product name** | `notchtap` | Mac-specific, short, action-oriented, unambiguous. Replaces all references to `crossbar-notify`, `push.sh`, and `mac-notification-nudge` in commands. |
| **HTTP port** | `127.0.0.1:9789` | Unassigned in IANA, unlikely to collide. If in use at startup, the app exits with a clear error rather than silently falling back. |
| **Swift ↔ Rust integration** | `notchtap-detect` CLI tool (stdout JSON) | Called by Rust via `std::process::Command`. Avoids FFI complexity entirely. Isolated and testable. |
| **Animation engine** | CSS keyframes (v1) | No dependency for a single generic template. Framer Motion may be evaluated at v2 time if the per-event-type table needs orchestrated sequences. |
| **macOS minimum version** | macOS 13 (Ventura) | `SMAppService.mainApp.register()` requires it. If a target machine runs macOS 12 or older, the login-item registration method needs `SMLoginItemSetEnabled` instead. |
| **Always-on-top** | v1 day-one (not deferred) | A notification overlay buried under other windows is useless. Click-through stays deferred. |
| **Posture module** | Future, not in v2 scope | Tracked idea with no committed timeline. Removed from v2 exit criteria. |

---

## `ARCHITECTURE.md` — Changes

### §0 — Phased Scope Table
- **No changes.** Still valid.

### §1 — What We're Building (v1)
- **No changes.** Still valid.

### §2 — System Architecture
- **No changes.** Still valid.

### §3 — Notification Queue Design
- **Added:** Full default values table for v1:

| parameter | default | notes |
|---|---|---|
| `ttl` | `8` seconds | time from enter-complete to exit-start |
| `max_concurrent` | `3` | visible stack items; excess wait in queue |
| `max_queued` | `50` | hard cap on waiting items; new pushes return `429` when exceeded |
| `enter_duration` | `300` ms | animation in |
| `exit_duration` | `300` ms | animation out |
| `queue_overflow` | reject with `429` | prevents unbounded memory growth if the UI is stuck |

- **Rationale:** Prevents the "what should the TTL be?" question during implementation. Easy to change later.

### §4 — Animation System
- **Changed:** "react + framer motion (or css keyframes)" → **"css keyframes"** for v1.
- **Added:** v2 may evaluate Framer Motion at v2 time if the config table needs orchestrated sequences.
- **Rationale:** A single simple animation doesn't justify a dependency. Lock the choice now so the executor doesn't flip-flop.

### §5 — Cross-Device Behaviour
- **Added:** **Integration pattern** paragraph: the Swift code is compiled as a tiny standalone CLI tool (`notchtap-detect`) that prints JSON to stdout. The Rust core calls it via `std::process::Command`. No FFI.
- **Rationale:** The original doc said "small Swift shim" but never specified how it connects. FFI is painful; a CLI boundary is clean and testable.

### §6 — Always-On Background Behaviour
- **Added:** `SMAppService.mainApp.register()` requires **macOS 13+**.
- **Added:** `always-on-top` (`setAlwaysOnTop` / `NSWindowLevel.floating`) as a **v1 day-one requirement** (moved out of the deferred list).
- **Changed:** Posture module label from **"v2-optional"** → **"future, not v2 scope"**.
- **Rationale:**
  - macOS minimum version matters because `SMAppService` is unavailable on macOS 12.
  - Always-on-top is as critical as visibility — it should not be deferred.
  - Posture module was ambiguously in v2 but not in v2 exit criteria.

### §7 — CLI Push (v1's Actual Notification Source)
- **Added:** **Default port** paragraph: `127.0.0.1:9789`. If the port is in use at startup, the Rust core exits with a clear error. The user can override via the config file.
- **Changed:** cmux command string from `crossbar-notify` → `notchtap`:
  ```
  notchtap --title "$CMUX_NOTIFICATION_TITLE" --subtitle "$CMUX_NOTIFICATION_SUBTITLE" --body "$CMUX_NOTIFICATION_BODY"
  ```
- **Rationale:** The binary name was a placeholder. The port needed a specific value.

### §8 — Tech Stack Recommendation
- **Changed:** "~100s of mb" → "hundreds of mb" (typo).
- **Changed:** "dx" → "developer experience (dx)" on first use.
- **Rationale:** Minor typo fixes.

### §9 — Distribution / Install
- **No changes.** Still valid.

### §10 — Configuration & Settings (NEW)
- **New section.** v1 config lives at `~/.config/notchtap/config.toml` (or JSON).
- **v1 fields:**

| field | default | description |
|---|---|---|
| `port` | `9789` | local HTTP listener port |
| `default_ttl` | `8` | seconds per notification |
| `max_concurrent` | `3` | visible stack items |
| `max_queued` | `50` | waiting items before rejecting |

- **v2 may add:** ESPN league list, cmux integration on/off, posture module on/off.
- **API keys:** separate env var / secret file — never committed, never pasted into chat.
- **Config file read:** once at startup. Changes require a restart in v1; a file-watcher or settings UI is a v2+ convenience.
- **Rationale:** A background utility needs at minimum a config file. Zero mention of this in the original docs.

### §11 — Logging & Observability (NEW)
- **New section.**
- **Rust core:** `tracing` (already pulled in by Tauri/Axum). Write to a rotating log file at `~/.local/share/notchtap/logs/notchtap.log` (or `~/Library/Logs/notchtap/` on macOS). Rotate at 10 MB, keep 3 backups. Log level `info` in release, `debug` in dev.
- **Frontend errors:** Any React error boundary catch or animation failure logs back to the same file via a Tauri command. The frontend never writes to disk directly.
- **macOS Console:** Optionally bridge `tracing` events to `os_log` via a small adapter, but file logs are the primary source of truth.
- **Rationale:** This is a background app — when something breaks, the user needs a log to read. Set up in v1, not as an afterthought.

### §12 — Multi-Display Edge Case (NEW)
- **New section.** The notch exists only on the built-in MacBook display. If the user has an external monitor and the menu bar is on the external display, the notification should still appear on the screen that has the notch (built-in) in notch mode, and on the primary screen in HUD mode.
- **v1 behavior:** Use Tauri's default screen (the one containing the menu bar). Acceptable for v1.
- **v2:** May query `NSScreen.screens` via the Swift shim to find the notch-bearing display explicitly.
- **Rationale:** The original docs never addressed this edge case. It will surface during v1 testing on a multi-monitor setup.

### §13 — Deduplication (NEW)
- **New section.** v1 has **no deduplication**. If cmux fires the same "agent needs input" notification twice in rapid succession, or a script loops with the same message, the queue will contain duplicates. This is acceptable for a personal tool with trusted, local sources.
- **If duplicate spam becomes a problem:** v2 can add a `(title, body)` hash deduplication window (e.g., 5 seconds). Tracked but not implemented until the pain is real.
- **Rationale:** Acknowledges the gap so the executor doesn't wonder "should I dedupe this?" during v1.

### §14 — IPC Security Model (NEW)
- **New section.** The frontend is **untrusted code running in a webview** — even though it's first-party. The Rust core treats it as a display-only consumer:
  - Frontend **receives** events via Tauri `emit` / `listen`
  - Frontend **does not invoke** commands back into Rust in v1
  - The Tauri capabilities file should reflect this: one event permission, no filesystem, no shell, no network from the frontend.
- **Rationale:** This boundary matters if the app ever processes untrusted content (e.g., WhatsApp messages from unknown senders in v3). Establishing the one-way data flow in v1 means v3 doesn't accidentally open a hole.

### §15 — Status
- **No changes.** Still valid.

---

## `CLAUDE.md` — Changes

### Project State
- **Changed:** `push.sh` → `notchtap` in the "treat as planned, not present" list.
- **Rationale:** The old `push.sh` reference was stale.

### Commands (once scaffolded)
- **Changed:** `./push.sh "title" "body"` → `./notchtap "title" "body"` (with port `127.0.0.1:9789`).
- **Added:** Consider adding a `justfile` (or `Makefile`) after scaffold with recipes: `dev`, `test-rust`, `test-web`, `test-all`, `build`, `push "title" "body"`. This prevents "oops I ran `vitest` from `src-tauri/`" errors.
- **Rationale:** The executor will run commands from two directories. A task runner prevents directory confusion.

### Architecture (once scaffolded)
- **Changed:** `127.0.0.1` → `127.0.0.1:9789` in the Rust core description.
- **Rationale:** Port is now locked.

### Naming
- **Changed:** Added `notchtap` as the preferred product name alongside the repo name.
- **Original:** "use only this repo's own name (`mac-notification-nudge`) or generic terms"
- **New:** "use the product name (`notchtap`), this repo's own name (`mac-notification-nudge`), or generic terms"
- **Rationale:** The product has a name now. It should be used consistently.

### IPC & Security (NEW)
- **New section.** Tauri v2 capabilities/permissions. Frontend is **receive-only** in v1 — listens for a single custom event (`notification-received`). No frontend-to-Rust invoke commands.
- The `src-tauri/capabilities/default.json` should be locked down to the minimum: one permission for the custom event channel, no filesystem, no shell, no network from the frontend.
- **Rationale:** The original docs never mentioned Tauri's security model. This is a hard requirement for the scaffold.

### Rust Error Handling (NEW)
- **New section.**
- **Library/internal modules** (queue, event bus, event types): use `thiserror` for structured, matchable error variants. Tests should be able to assert `matches!(err, MyError::QueueFull)`.
- **Application boundary** (main.rs, HTTP handlers, CLI entrypoint): use `anyhow` for ergonomic error propagation. The HTTP layer returns specific status codes (400 for malformed JSON, 429 for queue full, 500 for unexpected), but the internal error type doesn't need to leak into every function signature.
- **Rationale:** The original docs had no guidance on error handling. This split is standard and matches the testing strategy (unit tests match on variants; integration tests assert on HTTP status codes).

### Removed Duplicate
- **Fixed:** Removed a trailing duplicate "naming" paragraph that appeared at the end of the file. The file had the naming section twice.

---

## `IMPLEMENTATION_PLAN.md` — Changes

### Title
- **Changed:** `mac-notification-nudge` → `notchtap`.

### §0 — Ground Rules
- **Changed:** `macos` → `macOS` (capitalization).
- **Rationale:** Minor typo fix.

### §1.2 — Rust Core
- **Changed:** `127.0.0.1:<port>` → `127.0.0.1:9789`.
- **Rationale:** Port is now locked.

### §1.4 — CLI Push Mechanism
- **Changed:** `push.sh "title" "body"` → `notchtap "title" "body"`. Added the port explicitly: "default port `9789` on `127.0.0.1`".
- **Rationale:** Binary name and port are now locked.

### §1.5 — v1 Exit Criteria
- **Changed:** `./push.sh "test" "body"` → `notchtap "test" "body"`.
- **Rationale:** Binary name is now locked.

### §2.2 — cmux Notification Relay
- **Changed:** `crossbar-notify` → `notchtap` in the cmux command string.
- **Removed:** The parenthetical "(rename the binary/script to match this project's actual name before wiring this up)" — this is no longer a placeholder, it's locked.
- **Rationale:** The binary name was a placeholder. It's now `notchtap`.

### §2.3 — Animation Variety
- **No change to the text, but the implicit CSS keyframes lock is now in `ARCHITECTURE.md` §4.**

### §2.4 — Posture Module
- **Changed:** Label from "optional: posture module" → **"posture module (future, not in v2 scope)"**.
- **Added:** Explicit sentence: "**not part of v2** — a tracked idea with no committed timeline."
- **Removed:** Duplicate sentences that were carried over from the original text.
- **Rationale:** The original doc had posture listed as "optional" in v2 but it wasn't in v2 exit criteria. This clarifies it's not in v2 scope at all.

### §4 — Explicitly Deferred Polish
- **Fixed (2026-07-16 review pass):** `LSUIElement = true` + `SMAppService.mainApp.register()` was still listed here as deferred, directly contradicting `ARCHITECTURE.md` §6, which locks it as a v1 day-one requirement. This was flagged during the blind review but not actually corrected in the file at the time — that gap has now been closed. The bullet is removed from the deferred list and replaced with a pointer note to `ARCHITECTURE.md` §6. The deferred list itself (notch-precise positioning, click-through, real app icon) is otherwise unchanged.

### §5 — Verification Checklist
- **No changes.** Still valid.

### §6 — Open Items to Resolve Before Starting
- **Removed:** "pick this project's real cli/binary name" — this is now resolved (`notchtap`).
- **Kept:** "confirm actual macOS username / home path on the target machine" — still relevant.
- **Removed duplicate entry:** The item about confirming home path was duplicated in the original file. The duplicate was removed.
- **Rationale:** The binary name was the last open item.

---

## `TESTING_STRATEGY.md` — Changes

- **Correction (2026-07-16 review pass):** this file was originally marked "no changes needed," but that missed two new testable surfaces introduced by the decisions locked in this same round:
  - §4.1 and §4.3 now include an example case for the `429` queue-overflow behavior (`max_queued = 50`, locked in `ARCHITECTURE.md` §3) — there was no test case for it before.
  - §4.4 now covers the `notchtap-detect` subprocess/stdout-JSON parsing boundary (malformed JSON, non-zero exit, binary not found on `PATH`) — a surface that only exists because §5's swift↔rust integration pattern (CLI + stdout JSON, no FFI) was itself new in this round. It hadn't been retrofitted into the test plan until now.
- **Unaffected:** The animation framework choice (CSS keyframes) is locked in `ARCHITECTURE.md` §4, but `TESTING_STRATEGY.md` §4.6 already said "manual only, v1" for animation rendering, which is correct regardless of the specific animation library — no change needed there.

---

## `README.md` — New File

- **Created:** `README.md` with:
  - What the project does (summary of all features, v1/v2/v3)
  - Tech stack (Rust + Tauri, React + TS, CSS keyframes, Swift shim, Axum)
  - Quick start commands (dev, test, trigger notification)
  - Project doc index (links to all 4 docs with one-sentence descriptions of each)
  - Scope reminders (macOS only, personal use, clean-room build)
- **Rationale:** The repo had no README. For a project built on two machines and revisited months later, this is essential.

---

## `BLIND_REVIEW.md` — Updated

- **Original:** Listed ~15 issues with status "to resolve".
- **Updated:** All items marked as **addressed** with specific file locations where the change was made. New sections added to the architecture are documented. Typos fixed are listed. The review now serves as a closed checklist.
- **Rationale:** Tracks what was done so the executor can verify nothing was missed.

---

## Files in the Repo (Final State)

```
ARCHITECTURE.md          — 390 lines  — Architecture, decisions, new sections 10-14
CLAUDE.md                — 121 lines  — Guidance for Claude Code, new IPC & error handling
IMPLEMENTATION_PLAN.md   — 189 lines  — Execution plan, name/port locked, posture out of v2
TESTING_STRATEGY.md      — 203 lines  — Unchanged (already aligned)
README.md                —  69 lines  — New: overview, quick start, doc index
BLIND_REVIEW.md          — 131 lines  — Updated: all items addressed
```

---

## Addendum — 2026-07-16 review-and-fix pass

A second reviewer (independent of the one that produced everything
above) checked this changelog against the actual file contents rather
than taking the "ALL ITEMS ADDRESSED" claims at face value. Three
claimed-done items weren't actually done, plus one pre-existing
document corruption unrelated to this round's edits:

1. **`ARCHITECTURE.md` was corrupted at the end of the file.** A stray
   `## 15. status` heading was immediately followed by an orphaned
   duplicate fragment of §9's last sentence, with the real closing
   section still labeled `## 10. status` right after it. Fixed: the
   orphaned fragment is deleted, the real closing section is now
   correctly numbered `## 15. status`.
2. **The `LSUIElement`/`SMAppService` contradiction flagged in §4 above
   was never actually applied to `IMPLEMENTATION_PLAN.md`.** The
   original file still listed it as deferred. Now fixed directly in
   that file — see the corrected §4 entry above.
3. **`BLIND_REVIEW.md` item 3.7 claimed the ESPN API graceful-failure
   requirement was documented in `ARCHITECTURE.md` §7 — it wasn't,
   anywhere.** Added to `IMPLEMENTATION_PLAN.md` §2.1 instead (the
   actual home for ESPN poller detail): the endpoint is undocumented
   public and best-effort, so the poller must fail gracefully (no
   crash, log a warning, skip the cycle) rather than take the app down.
   `BLIND_REVIEW.md`'s citation is corrected to point at the right file.
4. `TESTING_STRATEGY.md`'s "no changes needed" claim was inaccurate —
   see the corrected section above.
5. `ARCHITECTURE.md` §3's default-values table had been inserted mid
   bullet-list, splitting the "concurrency" and "lifecycle per item"
   bullets apart. Moved to sit after both, as its own labeled block —
   cosmetic, but was visibly a leftover edit artifact.

no undocumented decisions remain, and — as of this pass — no
documented-but-not-actually-applied ones either.

## Next Step for the Executor

Run `npm create tauri-app@latest` with the React + TypeScript template, confirm `npm run tauri dev` opens a blank window, then proceed to `IMPLEMENTATION_PLAN.md` §1.2.