# consensus log — v1 (phase 1) plan verification

- **date**: 2026-07-16
- **question**: verify the v1 plan (engine + queue + one animation + cli push) — is it factually sound, effective, and on track for its stated goal ("prove the pipe end to end: a terminal command produces a visible animated notification on both machines")?
- **reviewed by**: openai/gpt-5.6-sol (for) + anthropic/claude-sonnet-5 (against) — both completed, one round.
- **inputs**: condensed brief from `docs/ARCHITECTURE.md`, `docs/IMPLEMENTATION_PLAN.md`, `docs/TESTING_STRATEGY.md`, `docs/V1_TECHNICAL_SPEC.md`, plus pre-consensus web verification (july 2026) of three load-bearing claims.

## pre-consensus research (own verification)

1. **factual error confirmed** — `V1_TECHNICAL_SPEC.md` §6 claims `tauri-plugin-autostart` "wraps the macOS 13+ SMAppService API". false: the plugin uses AppleScript (System Events, triggers TCC/Automation popups) or a LaunchAgent plist. SMAppService needs `smappservice-rs` or custom swift. sources: [plugin repo](https://github.com/tauri-apps/tauri-plugin-autostart), [gethopp writeup](https://www.gethopp.app/blog/rust-app-start-on-login), [autostart plugin src](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/autostart/src/lib.rs).
2. **cmux notification command confirmed real** — settings > app > notification command, `CMUX_NOTIFICATION_TITLE`/`CMUX_NOTIFICATION_BODY` env vars, runs via `/bin/sh -c`. sources: [cmux docs](https://cmux.com/docs/notifications), [mintlify mirror](https://manaflow-ai-cmux.mintlify.app/features/notifications).
3. **Info.plist merge / LSUIElement confirmed** in tauri v2; `ActivationPolicy::Accessory` is the programmatic alternative that also covers `tauri dev` (unbundled binary, no Info.plist). sources: [tauri v2 macOS bundle docs](https://v2.tauri.app/distribute/macos-application-bundle/), [tauri discussion #6038](https://github.com/tauri-apps/tauri/discussions/6038).

## verdict — gpt-5.6-sol (for), confidence 9/10

on track; no fundamental blocker. tauri v2 + axum-on-loopback + react + css keyframes all sound. corrections needed before it's reliable on both machines:

- **strongest risk: listener-registration race.** tauri events are transient — a `/notify` arriving before the webview registers its listener returns HTTP 200 with nothing rendered. wants a readiness gate (promote only after webview ready) or state recovery.
- autostart claim wrong (agrees with research); autostart isn't needed to prove the pipe anyway.
- `notchtap-detect` "on PATH" unreliable for GUI/login-item launches (minimal PATH: `/usr/bin:/bin:/usr/sbin:/sbin`) — bundle as tauri sidecar or resolve an absolute path.
- `safeAreaInsets.top` needs correct display selection; not a permanent machine classification (external displays, rearrangement).
- suggests notch detection could be deferred entirely in v1 since positioning is plain top-center — no observable payoff yet.
- verify `core:event:default` actually grants frontend listen under the scaffolded tauri version.
- loopback ≠ auth boundary against other local processes (fine for single-user, document it); add request-body size limits.
- estimate: bare pipe MVP 2–5 focused days; the macOS integration extras expand that substantially.

## verdict — claude-sonnet-5 (against), confidence 8/10

core pipe sound; "not ready" because the macOS integration layer is the weak point:

- autostart: "a locked architectural decision that is currently unimplementable as written." proposes a small swift cli (`notchtap-login`) calling `SMAppService.mainApp.register()` directly — consistent with the existing subprocess pattern.
- no code-signing/gatekeeper story for moving the build to the mac mini. *(partially pre-answered: `ARCHITECTURE.md` §9 covers quarantine-free transfer + `xattr -cr` — wasn't in the brief. the genuinely open piece is `notchtap-detect`'s deployment on the second machine, which spec §14 admits.)*
- silent hud fallback means v1 could "pass" while notch detection never worked — false-positive baked into the acceptance test; wants notch-mode confirmation in exit criteria.
- LSUIElement unverifiable in dev mode → late-discovery risk.
- macOS background/login-item layer likely 2–3x the effort the "locked, day-one" framing implies.

## agreements (all three: research + both reviewers)

1. core design (rust-owned queue, render-only frontend, axum/oneshot testing, subprocess-not-ffi swift boundary, capability lockdown) is sound and well-specified. plan is on the right track.
2. the autostart factual error is real and must be fixed as a *decision* edit in `ARCHITECTURE.md` §6 (per that doc's own rules).
3. `notchtap-detect` distribution (PATH under GUI launch + second-machine install) needs a deliberate answer — sidecar or absolute path.
4. LSUIElement needs `set_activation_policy(Accessory)` alongside the Info.plist key to cover dev mode.

## disagreements & reconciliation

- **defer notch detection (gpt) vs gate exit criteria on notch-mode (sonnet).** reconciled toward gpt with a nod to sonnet: keep the detector (cheap, unit-tested, needed for v2, and `IMPLEMENTATION_PLAN.md` §4 already defers notch-*precise positioning* deliberately), but log the detected mode prominently at startup so a silent hud fallback is visible, not gated.
- **gatekeeper gap (sonnet).** partially a brief artifact — `ARCHITECTURE.md` §9 already answers it. action reduced to: reference §9's transfer method in the v1 manual checklist.

## final recommendation

**on the right track — proceed, after five concrete edits:**

1. **fix `ARCHITECTURE.md` §6 + `V1_TECHNICAL_SPEC.md` §6 (autostart)**: drop the false plugin claim; pick the mechanism. recommended: swift subprocess helper calling `SMAppService` (matches the existing `notchtap-detect` pattern), or accept LaunchAgent via the plugin for a personal tool. decision edit, needs chetan's call since §6 is locked.
2. **close the listener-ready race**: gate queue promotion (or axum bind) on a rust-side webview page-load/ready signal — keeps the frontend receive-only. add a test case: pre-ready POST must not vanish.
3. **`notchtap-detect` via absolute path or tauri sidecar**, not `PATH`; add its second-machine build/install step to the plan (spec §14 already flags this open).
4. **add `set_activation_policy(Accessory)`** in `main.rs` alongside the Info.plist key.
5. **log the detected presentation mode at startup** (info level) so hud-fallback can't silently mask a broken detector; add "check the log shows notch mode on the macbook" to the manual checklist.

minor/optional: request-body size limit on `/notify`; note that loopback is not an auth boundary; clarify max_queued excludes visible items.
