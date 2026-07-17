## review log — cmux origin + rotation-order feature — 2026-07-17

**setup**: 2 reviewers via `/review`. Original default pair
(`moonshotai/kimi-k2.7-code`, `z-ai/glm-5.2`) did not resolve in PAL's
live model list (confirmed via `listmodels` — registry is stale, as the
skill's own flagged note warned). User chose the substitute pair:
`openai/gpt-5.1-codex` (for) + `google/gemini-2.5-pro` (against).

### round 1

**executor's diff under review**: the full working-tree diff (excluding
`CLAUDE.md`, owned by a concurrent, unrelated session) for the
per-source `Priority`/rotation-seconds config feature plus the `Cmux`
origin/settings-tab extension — `CONTEXT.md`, `docs/ARCHITECTURE.md`,
`notchtap` (CLI script), `src-tauri/src/{config,event,http,settings,
lib,poller,rss_poller,notifier}.rs`, `src/settings/{SettingsApp.tsx,
SettingsApp.test.tsx,settings.css}`. ~1780 lines. Full patch was
attached verbatim to both reviewer prompts (poller.rs/rss_poller.rs
mechanical parameter-threading and the full frontend diff were
summarized rather than pasted in full for length, with an offer to
supply them by filename on request — neither reviewer asked).

**reviewer 1 (openai/gpt-5.1-codex, for)**: **needs-changes**.
Top findings:
1. **(High) Regression for installs with a customized `default_ttl`.**
   Before this diff, `default_ttl` was the one TTL knob for football
   and any `/notify` push (manual or cmux). This diff gives football
   and cmux their own `espn_ttl_secs`/`cmux_ttl_secs` fields, both
   hardcoded to default `8` rather than inheriting whatever
   `default_ttl` the install had already set. A user who'd previously
   tuned `default_ttl` away from `8` would see football/cmux silently
   revert to `8` on upgrade — violating the stated "no behavior change
   for existing installs" goal. Suggested fix: inherit `default_ttl`
   into the new fields when they're absent from the file, or a config
   migration.
2. **(Medium) `rotation_order`'s default order changes cmux/manual
   tie-break behavior** for any install already running both,
   since Cmux now outranks Manual by default instead of both sharing
   one arrival-order FIFO bucket. Flagged as intentional-but-
   unannounced.
   API design (RequestSource narrowing, permutation validation): both
   judged sound, no security issues. Confidence 6/10 — explicitly
   flagged uncertainty about whether inheritance/migration logic
   existed elsewhere unshown to it (it does not).
   Test gaps: no test pins "manual uses default_ttl, cmux uses
   cmux_ttl_secs specifically, not vice versa"; CLI's
   `${CMUX_NOTIFICATION_BODY+x}` path has no automated coverage
   (consistent with this repo's existing "cmux relay is manual-only by
   design" testing policy, not called out as an oversight by the
   reviewer beyond a passing mention).

**reviewer 2 (google/gemini-2.5-pro, against)**: **approve** (9/10
confidence). Judged the `RequestSource` narrowing as a "textbook
example of enforcing trust boundaries at the type level," the
`rotation_order` permutation check as correct and efficient, the CLI's
`${CMUX_NOTIFICATION_BODY+x}` check as robust, and stated backward
compatibility is preserved "as demonstrated in config.rs (lines
154-159) and http.rs (line 134)" — this reasoning addresses "does an
omitted `priority`/`source` field resolve to the same value as
before" (yes) but does not appear to engage with reviewer 1's specific
scenario (an install that had already customized the *pre-existing*
`default_ttl` field away from its own default before this feature
shipped).

**disagreement surfaced**: yes. Reviewer 1: needs-changes on the
`default_ttl`-inheritance point specifically. Reviewer 2: approve,
no changes needed, did not address that specific scenario. Per the
skill's rule, no arbiter was called — both full verdicts were
presented to the user directly.

**relevant fact for the user's decision** (executor's own observation,
not a ruling): the actual current `~/.config/notchtap/config.toml` on
this machine has never set `default_ttl` — it's absent from the file,
so it's on the default (`8`) already. Reviewer 1's regression scenario
does not manifest for the current real install today. It would affect
any install (including a future one) that had explicitly tuned
`default_ttl`.

**user decision**: user chose to fix reviewer 1's findings rather than
ship as-is, even though the specific regression doesn't manifest on
the current real install.

**model substitutions**: `moonshotai/kimi-k2.7-code` → `openai/gpt-5.1-codex`,
`z-ai/glm-5.2` → `google/gemini-2.5-pro` (both originals unresolvable in
PAL's live model list at review time).

**action taken**:
1. `Config::parse` (`src-tauri/src/config.rs`) now re-parses the source
   TOML as a raw `toml::Table` alongside the normal typed parse, purely
   to detect which keys the file itself set (serde's whole-struct
   `#[serde(default)]` can't express "inherit sibling field X" — only
   "use `Config::default()`'s value" — so this was the only way to
   distinguish "field absent" from "field explicitly set to its default
   value"). When `espn_ttl_secs`/`cmux_ttl_secs` are absent from the
   file, they now inherit the file's effective `default_ttl` instead of
   their own hardcoded `8`. Fresh installs (no `default_ttl` in the
   file either) are unaffected — `default_ttl` resolves to its own
   default of `8` either way, same numeric result as before this fix.
2. `default_rotation_order()` reordered to `[Football, Manual, Cmux,
   News]` (Manual ahead of Cmux) — addresses reviewer 1's literal
   concern, though noted in a code comment that it's a no-op at default
   priorities (Football/Cmux both High, Manual Medium, News Low —
   Cmux/Manual never share a tier unless the user manually equalizes
   priorities).
3. Three new `config.rs` tests:
   `espn_and_cmux_ttl_inherit_a_customized_default_ttl_when_absent`,
   `explicit_espn_or_cmux_ttl_is_not_overridden_by_inheritance`,
   `absent_default_ttl_still_yields_the_shared_default_of_eight`. Two
   existing tests' hardcoded default `rotation_order` expectations
   updated to the new order.
4. Verified: `cargo test` 209 passed (was 206 — 3 new), 0 failed, 3
   doctests; `npx vitest run` 60 passed (unaffected, rust-only fix);
   `npx tsc --noEmit` clean.

No second review round was run on the fix itself — the change is small,
mechanical, and directly targets the exact finding reviewer 1 raised;
re-running the full panel was judged not worth the cost for this scope.
If the user wants a round 2, ask.
