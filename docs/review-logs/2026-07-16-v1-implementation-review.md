# review log — v1 implementation (full code review) — 2026-07-16

**setup**: 2 reviewers, `moonshotai/kimi-k2.7-code` (for) + `z-ai/glm-5.2` (against)
**constraint**: user hard rule — no git commits/pushes without explicit say-so; findings applied to files only.

## round 1

**executor's diff/summary**: full v1 implementation reviewed as-is on disk (irrespective of commits): rust core (lib.rs wiring incl. listener-ready gate via on_page_load + Once, 250ms heartbeat, tray pause/resume with blocking_lock, queue with promoted-accumulator emit rule, http 200/202/400/413/429/500, config at ~/.config, presentation subprocess with hud fallback, smappservice login item, custom size-rotating logger), frontend (useVisibleNotifications render-state hook, single css template), CLI sh script (jq+curl, flags-only), swift notchtap-detect. Verification state at review time: cargo test 35/35, vitest 5/5, builds clean, live-verified on the mac mini (burst, pause/resume, real cmux relay). Key prod code inlined verbatim in the reviewer prompt (queue.rs, http.rs, lib.rs, useVisibleNotifications.ts) after attempt 1 failed on file-embedding.

**run notes / failures**: attempt 1 — kimi-k2.7-code returned `files_required_to_continue` (relevant_files didn't reach it); retried with code inlined per mid-run-failure rule. attempt 2 step 2 — pal transport dropped mid-call consulting glm-5.2, continuation thread lost server-side; retried as a fresh thread with glm first (kimi not re-consulted; its completed verdict carried from the prior thread). Both reviewers ultimately completed.

**reviewer 1 (moonshotai/kimi-k2.7-code, for)**: **approve** (confidence 8/10) — "v1 is correct, spec-compliant, and ready to ship as the local-only macOS HUD; no blocking defects." Correctness: ttl-from-promoted_at, pause-gates-promotion-only, fifo fast-path guard, exactly-once emit via take_promoted all verified correct. Flags (non-blocking): (1) residual listener-attach race — React's listen() registers async after PageLoadEvent::Finished, so a push in that sub-second gap can 200-but-drop; webview reload loses all visible state with no re-sync; wants post-v1 ready-handshake/state-sync + documentation now. (2) frontend ttl measured from webview receipt, not rust promotion instant — drift under IPC delay/throttling (documented §8 trade-off, "real operational caveat"). (3) no local auth on loopback — appropriate for v1, should be documented.

**reviewer 2 (z-ai/glm-5.2, against)**: **needs-changes** (confidence 7/10). Findings: (1) **sleep/wake stale-card bug (top finding)** — webview setTimeout freezes during sleep while (per its analysis) rust's monotonic clock expires the item; no evict signal → a card could linger long after wake. Wants wake-detection or periodic revalidation in the frontend. (2) **emit-failure-after-enqueue permanently drops the item from the frontend's view** — take_promoted already drained it, no retry — violates the exactly-once-emit guarantee in the failure mode. (3) **blocking_lock in the tray handler is fragile** — safe only while menu events stay off tokio worker threads; a future refactor deadlocks silently; wants try_lock/channel or a guard. (4) `std::process::exit(1)` from the spawned server task skips tauri cleanup/log flush (minor). (5) dispatch() runs before enqueue — side-effect risk if dispatch ever does I/O (couldn't see the body; flagged as suspicion). (6) no auth token on loopback — acceptable single-user, injection vector on shared Macs; suggests optional bearer token. Explicitly cleared after examination: pause/resume interleaving, on_page_load+Once gate, cleanup's live-map iteration, serde handling.

**executor reconciliation of specific claims**:
- glm's (5) dispatch-before-enqueue: not a bug — dispatch() is a pure match returning Ok for Generic, no I/O/side effects (glm hedged precisely because it wasn't inlined).
- glm's (1) premise is partially off on macOS specifically — rust `Instant` uses mach_absolute_time, which also suspends during system sleep, so rust-vs-webview divergence under *sleep* is smaller than claimed; but the *timer-throttling-while-occluded* variant of the same staleness bug is real, and a wall-clock deadline recheck fixes both cheaply.
- glm's (2)(6) and kimi's (1)(2)(3) are acknowledged/documented trade-offs, real but deliberate for a single-user v1.

**disagreement surfaced**: yes — approve (kimi) vs needs-changes (glm). Both full verdicts presented to the user side by side with executor's recommendation: apply three cheap hardening fixes (frontend wall-clock deadline recheck; app_handle.exit(1) instead of process::exit in server task; debug_assert runtime-thread guard before blocking_lock), treat the remainder as documented limitations.

**user decision**: <pending at time of writing — updated below>

**model substitutions**: none (retries only, same models).

**action taken**: <pending user decision>
