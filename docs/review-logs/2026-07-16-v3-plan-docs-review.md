## docs-review log — v3 implementation plan (IMPLEMENTATION_PLAN.md §3) — 2026-07-16

**setup**: 2 reviewers via PAL `consensus` (invoked over the newly configured opencode MCP endpoint). the skill's default pair (`openai/gpt-5.6-sol`, `anthropic/claude-sonnet-5`) does not resolve in PAL's openrouter registry (stale `conf/openrouter_models.json`, the skill's own flagged open question) — substituted with the pair used by this repo's previous docs-review runs: `openai/gpt-5.2` (for) + `anthropic/claude-opus-4.5` (against).

reviewed by: openai/gpt-5.2 (for) + anthropic/claude-opus-4.5 (against) — both completed.

### round 1

**executor's document** (docs/IMPLEMENTATION_PLAN.md §3, uncommitted working-tree state; companion files V3_TECHNICAL_SPEC.md, CONTEXT.md, ARCHITECTURE.md attached to reviewers):

```markdown
## 3. v3 — outbound connectors

decisions locked 2026-07-16 (grilling session; code-level contract in
`docs/V3_TECHNICAL_SPEC.md`):

- **the seam sits at acceptance, not promotion**: once a push passes
  validation and `enqueue` succeeds, it fans out to every connector.
  the queue (cap/ttl/pause/promotion) is a *display* concern owned by
  the overlay path alone — pausing the overlay must not silence the
  phone (that's when outbound matters most). rejected pushes (`429`)
  reach no connector.
- **honest asymmetry**: the `Notifier` trait is outbound-only. the
  overlay is not a notifier; the queue keeps owning the http contract
  (200/202/429). a connector's outcome never influences the response
  to the pusher.
- **worker-per-connector**: each connector = one bounded mpsc channel
  (~64) + one long-lived worker task sending serially. acceptance does
  `try_send`; channel full → drop + warn. never block, never deliver
  a stale backlog.
- **telegram first** (botfather bot + `sendMessage`, one rest call, no
  approval process). whatsapp/twilio demoted to "maybe later" — the
  twilio sandbox's 72h re-join and meta's template rules make it a
  poor fit for an always-on personal notifier; re-evaluate only if
  telegram proves insufficient. (reopens the earlier "whatsapp
  preferred" line, 2026-07-16.)
- **no routing, no presence-gating in v3**: every accepted event goes
  to every enabled connector, always — even while at the mac. a
  per-connector event-type filter and away-detection are both
  deliberate v3.5 candidates, not v3 scope.
- **failure semantics**: 10s send timeout; one retry after ~5s, then
  drop. formatting rejections (`400`) resend once as plain text
  instead of retrying the same broken payload.
- **message format**: per-event-type templates over telegram html mode
  (3-char escaping), unknown types fall back to generic — the same
  data-not-code move as the frontend's animation table.
- **secrets**: bot token + chat id in `~/.config/notchtap/secrets.toml`
  (checked for `600` perms), never in `config.toml`, never committed,
  never pasted into chat. no env vars — login-item launches don't
  inherit shell env. missing secrets = connector disables itself with
  a warning; the app runs overlay-only.

### 3.1 v3 exit criteria
- `cargo test` passes with the new notifier suite: `format_message`
  per type + escaping, `RetryDecision` rules, channel-full drop, and
  the acceptance fan-out test (accepted event reaches the channel,
  rejected doesn't) — see `TESTING_STRATEGY.md` §4.9
- wiremock covers the send path (success / 400 / 5xx); no live
  telegram call in any test, ever
- manual (§6): one real end-to-end telegram message on the mac mini;
  overlay behaviour unchanged with the connector enabled and with
  secrets absent

---

## 4. v4 — github, ci, expanded test suites — ✅ done 2026-07-16

```

**reviewer 1 (openai/gpt-5.2, for)**: needs-changes —

## Verdict
Strong direction and largely consistent with the companion docs, but **needs-changes** to resolve a core semantic contradiction (“never deliver stale backlog” vs bounded buffering + slow retry) and to tighten v3 exit criteria around the locked decisions (paused `202` fan-out, secrets-perms/enablement, and connector opt-in).

## Analysis

### 1. Technical Feasibility
- The architecture is implementable with reasonable effort: bounded `mpsc` + one worker task per connector, `try_send` at acceptance, and wiremock-based HTTP tests are standard patterns.
- The plan’s seam placement (“acceptance, not promotion”) is coherent and matches the v3 spec’s call graph.
  - **Plan**: IMPLEMENTATION_PLAN.md **L202–L207** “…once a push passes validation and `enqueue` succeeds, it fans out…” (context_start_text: “**the seam sits at acceptance”, context_end_text: “reach no connector.”)
  - **Spec**: V3_TECHNICAL_SPEC.md **L27–L36** shows `/notify → … → queue.enqueue ──ok──► … for each connector: try_send(event)`.

### 2. Project Suitability (fit with existing architecture)
- This v3 plan is explicitly aligned with the glossary definition of “Connector”:
  - CONTEXT.md **L43–L47**: “receives every accepted Event… observes acceptance, not Promotion… failures never affect the pusher’s response.”
- The “honest asymmetry” decision also aligns with the v1/v2 security posture that the overlay/webview is display-only and shouldn’t own outbound effects.

### 3. User Value Assessment
- Clear value: outbound delivery when away or when overlay is paused. The plan’s rationale (“pausing overlay must not silence the phone”) is compelling and user-correct for an “always-on personal notifier.”
- Telegram-first is a pragmatic choice with low friction; the rationale is specific and credible:
  - IMPLEMENTATION_PLAN.md **L216–L220** cites Twilio sandbox churn + Meta templates as poor fit.

### 4. Implementation Complexity / Risks
**Main risk: “never deliver stale backlog” is not actually guaranteed by the described mechanism.**
- The plan claims: bounded channel + `try_send` + drop-on-full ⇒ “never deliver a stale backlog” (IMPLEMENTATION_PLAN.md **L212–L215**, excerpt: “channel full → drop + warn. never block, never deliver a stale backlog.”).
- But with the stated retry policy—10s timeout + retry after ~5s (IMPLEMENTATION_PLAN.md **L226–L228**)—a connector can process events slowly during network/API issues. This allows up to ~64 queued events (per connector) to sit and potentially be delivered minutes later (stale), while *newer* events are dropped due to full channel (“drop-newest” behavior).
- If “no stale backlog” is a true locked requirement, you likely need one of:
  - **Drop-oldest / keep-latest** queue semantics (ring buffer), or
  - **Age-based discard** (timestamp events; drop if older than N seconds for outbound), or
  - **Circuit-breaker / fail-fast** mode where worker quickly drops while unhealthy to avoid backlog accumulation.
- If the intent was “no unbounded backlog” rather than “no stale backlog,” the phrasing should be corrected to avoid a contract you can’t meet.

### 5. Alternative Approaches
- Keeping the worker-per-connector model is good (simplicity, isolation). The suggested tweak is not to change the topology, but to adjust buffering semantics (drop-oldest/age-based) to match the “freshness” goal.
- Another low-complexity alternative: channel capacity 1 + “latest wins” (store last event in an `Atomic`/mutex and notify worker), if you truly only care about the most recent alert during outages.

### 6. Industry Perspective / Best Practices
- Best-effort outbound sinks typically choose between:
  - **At-least-once** (queues + persistence) vs
  - **Freshness-first** (drop/backpressure, keep-latest).
- Your plan is explicitly freshness-first, which is reasonable for personal alerting—but then the backlog semantics should reflect that (see above).

### 7. Long-Term Implications
- Good: isolating secrets into a separate file with strict perms and disabling connector when missing keeps the app usable and reduces accidental leakage risk.
- One completeness gap: the plan omits the **opt-in config flag** and default-off behavior that the spec defines:
  - V3_TECHNICAL_SPEC.md **L93–L99** adds `[connectors.telegram].enabled = false`.
  - IMPLEMENTATION_PLAN.md §3 mentions secrets but doesn’t state enable/disable controls or default-off. This is likely important for rollout and predictable behavior.

### Exit Criteria Review (completeness & internal consistency)
What’s strong:
- Automated suite enumerated (formatting, retry rules, drop-on-full, fan-out accepted vs rejected) and wiremock-only approach (IMPLEMENTATION_PLAN.md **L238–L247**) is solid and testable.

What’s missing or under-specified relative to locked decisions/spec:
- The plan explicitly asserts “paused overlay must not silence the phone,” but **exit criteria do not explicitly require testing that paused (`202`) still fans out**. The spec does:
  - V3_TECHNICAL_SPEC.md **L127–L129**: “paused (`202`) push **does** (acceptance succeeded).”
- Secrets hardening: exit criteria don’t mention validating the **0600 perms check** or the “enabled=true but secrets invalid ⇒ disabled + warn” branch (spec **L109–L113**).
- Config/enablement: no exit check that telegram is **opt-in** (default off) and only spawns worker when enabled + secrets present (spec **L93–L99**, **L113–L114**).

## Confidence Score
8/10 - The documents are highly aligned and specific, and the main issues are clear textual/semantic mismatches plus exit-criteria gaps; confidence would be higher with TESTING_STRATEGY.md §4.9 visible to confirm it already covers paused-202 and secrets-perms cases.

## Key Takeaways
- Fix the contradiction: “never deliver stale backlog” is not guaranteed with a bounded channel + potentially slow retry; choose drop-oldest/keep-latest or age-based discard, or revise the claim.
- Tighten v3 exit criteria to explicitly include **paused (`202`) fan-out** (not just accepted/rejected), matching V3_TECHNICAL_SPEC.md **L127–L129**.
- Add plan-level coverage for **connector enablement (default off)** and **secrets file perms/invalid-secret behavior**, which the spec defines but the plan doesn’t gate.

**Explicit verdict: needs-changes.**  
Top reasons: (1) stale-backlog claim contradicts the described buffering+retry behavior, (2) exit criteria omit paused-`202` fan-out despite being a locked decision, (3) plan doesn’t gate connector opt-in + secrets-perms/enablement behavior that the spec relies on.

**reviewer 2 (anthropic/claude-opus-4.5, against)**: approve (one editorial clarification) —

## Verdict
The v3 section is internally consistent, well-reasoned, and properly cross-referenced with its companion documents—it should be approved with one minor clarification needed regarding the `202` fan-out behavior.

## Analysis

**Internal Consistency**
The section maintains strong internal logic. The core principle—"the seam sits at acceptance, not promotion"—is stated clearly and then consistently applied: pausing affects only the overlay path, connectors receive events at enqueue success, and the queue's display concerns (cap/TTL/pause) never gate outbound delivery. The worker-per-connector design (bounded channel, try_send, drop-on-full) aligns with the "never block" principle stated in the same breath.

**Completeness of Exit Criteria (§3.1)**
Exit criteria are well-specified and testable:
- Unit tests: `format_message`, `RetryDecision`, channel-full drop, acceptance fan-out (LINE 239-242: `cargo test` passes with the new notifier suite...)
- Integration: wiremock for send paths (LINE 243-244)
- Manual: real telegram message + overlay unchanged with/without secrets (LINE 245-247)

One gap: the exit criteria reference `TESTING_STRATEGY.md §4.9`, but this document isn't provided. If §4.9 doesn't exist yet, the exit criteria become unverifiable until that section is written. This is a documentation debt, not a plan flaw—acceptable if §4.9 is created before v3 build starts.

**Soundness of Locked Decisions' Rationale**
Each decision records *why* it was chosen:
- Telegram over WhatsApp: explicit reasoning (72h re-join, template rules)—LINE 216-221
- No routing/presence-gating: deliberately v3.5, not forgotten—LINE 222-225
- Secrets in file not env: login-item inheritance issue documented—LINE 232-236

The rationale is practical and defensible.

**Cross-Document Consistency**

*With V3_TECHNICAL_SPEC.md:*
- Seam location: matches (§1 of spec)
- Channel cap 64: matches (§2 of spec)
- Retry semantics (10s timeout, 5s delay, plain-text fallback): matches (§2/§3)
- Secrets path and permissions: matches (§4)
- Testing approach: matches (§5)

*With CONTEXT.md:*
- Connector definition (LINE 43-47) explicitly states connectors observe *acceptance*, not promotion—perfect alignment.
- "Connector's failures never affect the pusher's response"—matches "honest asymmetry" in the plan.

*With ARCHITECTURE.md:*
- §7 (LINE 260-267) documents the WhatsApp demotion and points to IMPLEMENTATION_PLAN.md §3—consistent.
- §10 (LINE 363) correctly notes secrets live separately from config.toml.

**One Potential Ambiguity**
The plan states "rejected pushes (`429`) reach no connector" (LINE 206-207), which is correct. But it also implies paused pushes (`202`) *do* fan out (pausing must not silence the phone). V3_TECHNICAL_SPEC.md §5 (LINE 128-129) confirms: "paused (`202`) push **does** (acceptance succeeded)."

However, the IMPLEMENTATION_PLAN text doesn't explicitly state that `202` responses fan out—it only says `429` doesn't. This is logically deducible but could be clearer. Consider adding: "Paused pushes (`202`) fan out normally; only capacity-rejected pushes (`429`) are silenced."

**What's Missing but Acceptable**
- No mention of connector enable/disable at runtime (tray toggle). Correct—§3 says no UI beyond what exists; consistent with ARCHITECTURE.md §6's bar for tray items.
- No structured logging requirements for connector events. Minor omission; ARCHITECTURE.md §11 covers logging generally, so this inherits.

## Confidence Score
8/10 - High confidence in internal consistency and cross-document alignment. Uncertainty: (1) cannot verify TESTING_STRATEGY.md §4.9 exists and matches, (2) the `202` fan-out behavior is implicit rather than explicit in the plan text itself.

## Key Takeaways
- **Approve with one clarification**: explicitly state that `202` (paused) pushes *do* fan out to connectors—the logic is correct but currently inferred.
- The testing crosswalk is well-designed; ensure §4.9 of TESTING_STRATEGY.md exists before starting v3 implementation.
- WhatsApp demotion is properly recorded with reopening history—good traceability.
- The "data-not-code" pattern (templates, not per-type code paths) is consistently applied across frontend animation and connector formatting—sound architectural principle.

**Verdict: APPROVE** (pending the minor `202` clarification, which is editorial, not structural).

**disagreement surfaced**: yes — needs-changes (for-stance) vs approve (against-stance). handed to the user per skill rules; no arbiter, no synthesized ruling.
**user decision**: adopt both — user chose option 1 (apply gpt-5.2's needs-changes findings *and* opus-4.5's editorial clarification).
**model substitutions**: openai/gpt-5.6-sol → openai/gpt-5.2 and anthropic/claude-sonnet-5 → anthropic/claude-opus-4.5 (default slugs unresolvable in PAL's stale registry; substitution pair matches this repo's prior docs-review precedent)
**action taken**: three edits to `IMPLEMENTATION_PLAN.md` §3 —
1. seam bullet: paused (`202`) fan-out made explicit; rejected set widened to `400`/`413`/`429` (opus-4.5's clarification).
2. worker bullet: "never deliver a stale backlog" overclaim replaced with the honest guarantee — *bounded and non-blocking, not fresh*; staleness up to a channel's depth under degraded network is now stated as accepted (gpt-5.2 finding 1).
3. §3.1 exit criteria: added config-gate tests — `enabled` default-`false` parse test, secrets loader disabling on missing file / non-`0600` perms (gpt-5.2 finding 2).
`V3_TECHNICAL_SPEC.md` checked for the same overclaim — its worker-loop wording ("never blocks... drop and warn") was already honest; no spec change needed.


### round 2

**executor's document** (updated §3 after round-1 fixes, uncommitted working-tree state):

```markdown
## 3. v3 — outbound connectors

decisions locked 2026-07-16 (grilling session; code-level contract in
`docs/V3_TECHNICAL_SPEC.md`):

- **the seam sits at acceptance, not promotion**: once a push passes
  validation and `enqueue` succeeds, it fans out to every connector.
  the queue (cap/ttl/pause/promotion) is a *display* concern owned by
  the overlay path alone — pausing the overlay must not silence the
  phone (that's when outbound matters most). paused pushes (`202`)
  fan out normally — acceptance succeeded; rejected pushes
  (`400`/`413`/`429`) reach no connector.
- **honest asymmetry**: the `Notifier` trait is outbound-only. the
  overlay is not a notifier; the queue keeps owning the http contract
  (200/202/429). a connector's outcome never influences the response
  to the pusher.
- **worker-per-connector**: each connector = one bounded mpsc channel
  (~64) + one long-lived worker task sending serially. acceptance does
  `try_send`; channel full → drop + warn. the guarantee is *bounded
  and non-blocking*, not *fresh*: acceptance is never delayed and the
  backlog is hard-capped at the channel depth, but under a degraded
  network up to a channel's worth of events can still be delivered
  late before drops kick in — accepted; the cap bounds the damage
  rather than eliminating staleness.
- **telegram first** (botfather bot + `sendMessage`, one rest call, no
  approval process). whatsapp/twilio demoted to "maybe later" — the
  twilio sandbox's 72h re-join and meta's template rules make it a
  poor fit for an always-on personal notifier; re-evaluate only if
  telegram proves insufficient. (reopens the earlier "whatsapp
  preferred" line, 2026-07-16.)
- **no routing, no presence-gating in v3**: every accepted event goes
  to every enabled connector, always — even while at the mac. a
  per-connector event-type filter and away-detection are both
  deliberate v3.5 candidates, not v3 scope.
- **failure semantics**: 10s send timeout; one retry after ~5s, then
  drop. formatting rejections (`400`) resend once as plain text
  instead of retrying the same broken payload.
- **message format**: per-event-type templates over telegram html mode
  (3-char escaping), unknown types fall back to generic — the same
  data-not-code move as the frontend's animation table.
- **secrets**: bot token + chat id in `~/.config/notchtap/secrets.toml`
  (checked for `600` perms), never in `config.toml`, never committed,
  never pasted into chat. no env vars — login-item launches don't
  inherit shell env. missing secrets = connector disables itself with
  a warning; the app runs overlay-only.

### 3.1 v3 exit criteria
- `cargo test` passes with the new notifier suite: `format_message`
  per type + escaping, `RetryDecision` rules, channel-full drop, and
  the acceptance fan-out test (accepted event reaches the channel,
  rejected doesn't, paused-202 does) — see `TESTING_STRATEGY.md` §4.9
- config gates are tested too: `connectors.telegram.enabled` parses
  with default `false` (outbound is opt-in per machine), and the
  secrets loader disables the connector on a missing file or non-`0600`
  permissions (unit tests against temp files)
- wiremock covers the send path (success / 400 / 5xx); no live
  telegram call in any test, ever
- manual (§6): one real end-to-end telegram message on the mac mini;
  overlay behaviour unchanged with the connector enabled and with
  secrets absent

---

## 4. v4 — github, ci, expanded test suites — ✅ done 2026-07-16

```

**reviewer 1 (openai/gpt-5.2, for)**: needs-changes (narrow) —

## Verdict
**Needs-changes** — the round-1 fixes are largely adequate and the v3 section is close to build-gating quality, but it still lacks one or two explicit *safety/robustness* gates (malformed/partial secrets handling, and a small wording/contract consistency tweak) that could otherwise let a “green” v3 still ship with a crashy or ambiguous connector enablement path.

## Analysis

### 1. Technical feasibility
- The core design is technically straightforward and appropriate for Rust/Tokio: a bounded `mpsc` per connector + long-lived worker is a well-trodden pattern.
- The revised “bounded, non-blocking, not fresh” guarantee is now honest and implementable. In §3, lines **213–220** (“**worker-per-connector**… backlog is hard-capped… can still be delivered late…”) the plan correctly describes the real behavior of bounded queues under network degradation.
  - **context_start_text:** “**worker-per-connector**”
  - **context_end_text:** “rather than eliminating staleness.”
- Explicitly stating the acceptance fan-out including paused `202` removes ambiguity. In §3, lines **202–208** (“paused pushes (`202`) fan out normally… rejected pushes (`400`/`413`/`429`) reach no connector.”) is a crisp, testable contract.
  - **context_start_text:** “**the seam sits at acceptance, not promotion**”
  - **context_end_text:** “reach no connector.”

### 2. Project suitability (fit with existing architecture/contracts)
- This aligns cleanly with the architecture glossary: connectors observe *acceptance* not *promotion* (see `CONTEXT.md` lines **43–47**, “Connector… observes acceptance, not Promotion…”), so §3’s seam decision is consistent with the project’s ubiquitous language.
- “Honest asymmetry” (implementation plan §3 lines **209–212**) matches the security posture in `ARCHITECTURE.md` §14 (frontend treated as untrusted; one-way data flow). Keeping connector results out of the `/notify` HTTP response is the right coupling.
- One small consistency nit: the locked decision bullet says “fans out to **every connector**” (§3 line **203**) but later scope says “every accepted event goes to every **enabled connector**” (§3 line **228**). It’s obvious what you mean, but build-gating docs should be mechanically unambiguous.

### 3. User value assessment
- Strong user value: outbound matters most specifically when the overlay is paused (“that’s when outbound matters most”), and your contract preserves that. This is a coherent “don’t miss alerts while deep focused” story.
- Telegram-first is pragmatic for personal tooling: minimal onboarding, no template bureaucracy.

### 4. Implementation complexity / risks
- The new exit criteria are materially better and mostly sufficient as gates:
  - notifier suite + fan-out tests (§3.1 lines **244–247**) and config gate tests (§3.1 lines **248–251**) meaningfully prevent regressions.
  - wiremock-only policy (§3.1 lines **252–253**) is the right reliability constraint.
- Remaining gap: secrets robustness is not fully gated.
  - You gate **missing file** and **non-0600 perms** (§3.1 lines **250–251**), but not **malformed TOML**, **missing keys**, or **wrong types** (e.g., `chat_id` numeric vs string). If the loader does anything like `unwrap()` on parse, v3 could “pass gates” but crash on startup in real use.
  - This matters because the plan’s promise is “connector disables itself with a warning; the app runs overlay-only” (§3 lines **240–241**). That promise should be backed by explicit tests.
- Minor operational risk: `drop + warn` on a full channel can log-spam during an outage (tight loops of accepts). Not necessarily a blocker, but consider rate-limiting warnings or aggregating counts.

### 5. Alternative approaches
- For v3, your approach is the simplest correct one. Alternatives (global shared queue, blocking send in handler, persistent disk queue) either violate non-blocking acceptance or add unnecessary durability/complexity for a personal notifier.

### 6. Industry perspective / best practices
- “Best-effort, bounded queue, don’t block ingest” matches common notification fan-out designs (ingest path decoupled from outbound sinks).
- Secrets-in-file with strict perms is a standard macOS-adjacent practice; avoiding env vars for login items is correct.

### 7. Long-term implications
- Worker-per-connector scales fine for a handful of sinks and keeps complexity localized.
- The explicit “no routing/presence gating in v3” prevents scope creep and preserves an easy v3.5 extension point.

## Confidence Score
**8/10** — high confidence the design and the round-1 fixes are sound and implementable; the remaining uncertainty is primarily about secrets parsing/validation behavior (which isn’t fully build-gated yet) and a small contract wording ambiguity.

## Key Takeaways
- The round-1 fixes are **substantively adequate**: bounded-not-fresh guarantee (§3 **213–220**), explicit paused-`202` fan-out (§3 **202–208**), and config gate exit criteria (§3.1 **248–251**) all improve correctness and testability.
- **Add build gates for malformed/partial secrets** to uphold the “disable-with-warning, overlay-only” promise (§3 **240–241**).
- Tighten wording to remove ambiguity: change “every connector” → “every **enabled** connector” in the seam bullet (§3 **202–204** excerpt: “fans out to every connector”).
- Optional hardening: rate-limit or aggregate “channel full → drop + warn” logs to avoid noisy failure mode during outages.

**Explicit verdict: needs-changes. Top reasons (1–3):**
1) Missing exit criteria/tests for **malformed or incomplete `secrets.toml`** (must disable connector without crashing).  
2) Minor **contract ambiguity** (“every connector” vs “every enabled connector”) in a locked decision section.  
3) (Optional) Consider **log spam mitigation** for channel-full drop warnings under sustained outage.

**reviewer 2 (anthropic/claude-opus-4.5, against)**: approve —

## Verdict
The v3 section is now technically sound and adequate as a build-gating plan; the three Round 1 issues have been addressed with appropriate specificity.

## Analysis

**Round 1 Fixes Assessment:**

1. **Bounded-not-fresh guarantee (LINE 215-220)**: The overclaim is fixed. The new language explicitly acknowledges staleness can occur ("up to a channel's worth of events can still be delivered late before drops kick in") and frames the guarantee correctly as bounding damage rather than eliminating it. This is honest and implementable.

2. **Config gate exit criteria (LINE 248-251)**: Now explicit and testable: `enabled` parses with default `false`, secrets loader disables on missing file or wrong permissions, unit tests against temp files. This directly addresses the missing acceptance criteria.

3. **202-fan-out behavior (LINE 206-208)**: Now explicit: "paused pushes (`202`) fan out normally — acceptance succeeded." This removes the ambiguity and aligns with V3_TECHNICAL_SPEC.md LINE 127-129's test case.

**Cross-document Consistency:**
- IMPLEMENTATION_PLAN §3 aligns with V3_TECHNICAL_SPEC.md on all material points
- CONTEXT.md LINE 43-47 Connector definition matches the described behavior
- ARCHITECTURE.md LINE 74-75 correctly defers connector details to this section

**Remaining Minor Observations (non-blocking):**
- The retry timing "~5s" is imprecise but acceptable for a spec (implementation will pin it)
- No explicit mention of graceful shutdown draining the channel, but this is implementation detail appropriate to defer

**What's Still Sound:**
- Seam placement at acceptance, not promotion — correct architectural choice
- Worker-per-connector isolation prevents cascading failures
- Secrets file with permission check is the right pattern for login items
- Exit criteria are now complete and verifiable

## Confidence Score
8/10 - High confidence the section is implementation-ready. The fixes directly address Round 1 findings, the exit criteria are now testable, and cross-document consistency holds. Minor uncertainty only around whether the "~5s" retry delay needs pinning before implementation starts (likely fine to defer).

## Key Takeaways
- All three Round 1 issues are adequately resolved with explicit, testable language
- The bounded-not-fresh reframing is the most important fix — it sets correct expectations
- Config gate testing criteria are now specific enough to write tests against
- The section is ready to serve as a build-gating plan

**Verdict: APPROVE**

Top reasons:
1. Bounded-not-fresh guarantee is now honest and implementable
2. Exit criteria explicitly cover config gates and secrets-loader edge cases
3. 202-fan-out behavior is unambiguous and test-covered

**disagreement surfaced**: yes — needs-changes (for-stance, two one-line residuals + one optional) vs approve (against-stance, implementation-ready). handed to the user.
**user decision**: <pending>
**model substitutions**: none (same substituted pair as round 1)
**action taken**: <pending user decision>
