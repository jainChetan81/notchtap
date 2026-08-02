# Plan 180: Tab-wire test backfill — validator rejection cases, the App seam, the identity parity pin, and one validator unification

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 7ca82d5..HEAD -- src/useStatusState.test.ts src/App.test.tsx src/lib/iconPresence.test.ts src/settings/sections/ShortcutsSection.tsx src-tauri/src/settings.rs src-tauri/src/tabs.rs src/components/IconStrip.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: 176, 177 (soft — the App seam test in Step 2 asserts post-fix mount behaviour; if they have not landed, write the seam test against current behaviour and note it)
- **Category**: tests
- **Planned at**: commit `7ca82d5`, 2026-08-02

## Why this matters

Plan 171 added new wire fields and a new rust↔TS identity list, and the
repo's own testing bar ("every example case for a component has a passing
test", `docs/TESTING_STRATEGY.md`) is met for the older wire families but
not the new ones:

- The four new `isValidStatusState` clauses (`agent.activeSessions`,
  `news.chargeFraction` with its `[0,1]` range — the only range check in
  the file — `news.chargeCount`, `news.isCharged`) have **zero rejection
  coverage**; the delta only added the fields to fixtures. A rust
  regression shipping `chargeFraction: 1.4` or a missing `agent` block
  would blank the whole status rail via the fallback with no test naming
  the cause.
- `useTabSelection`'s only production call site (`App.tsx`) has no
  integration test: dropping the `selectedTab` prop or subscribing to the
  wrong channel would degrade to "nothing ever selected" with a green
  suite.
- The five-tab identity set is hand-written in six places (rust
  `Tab::ORDER` / `from_prefix_digit` / `wire_label`; TS `Tab` union +
  `TAB_ORDER`; `lib.rs`'s `PREFIX_FOLLOWUPS` digits; `iconPresence.ts`;
  `icon-strip.css` selectors) with no parity gate — and drift fails in the
  safe-LOOKING direction: the TS validator coerces an unknown token to
  `selected: null`, silently.
- The two prefix-shortcut validators declare "exact sync" but use
  different whitespace classes (rust `char::is_whitespace` = Unicode
  `White_Space`, includes U+0085, excludes U+FEFF; JS `/\s/` excludes
  U+0085, includes U+FEFF), so a value can pass the UI and fail the save.

## Current state

`src/useStatusState.ts:222-238` — the validator's new clauses:

```ts
isNonNegativeInteger(agent.activeSessions) &&
...
typeof news.chargeFraction === "number" &&
news.chargeFraction >= 0 &&
news.chargeFraction <= 1 &&
isNonNegativeInteger(news.chargeCount) &&
typeof news.isCharged === "boolean" &&
```

`src/useStatusState.test.ts` — 32 `it(` blocks; the existing rejection
pattern to copy is e.g. `:135` "ignores a payload with a non-boolean
football gate or a malformed live match". The delta since `acdaeb0` only
inserted `agent: { activeSessions: 0 }` and
`news: { enabled: …, chargeFraction: 0, chargeCount: 0, isCharged: false }`
into fixtures.

`src/lib/iconPresence.test.ts` — thorough on all five sources but never
calls `iconPresenceFor(undefined)` (the early return at
`iconPresence.ts:55-63` is untested).

`src/App.test.tsx` — zero hits for `tab-selection`/`selectedTab`/
`IconStrip`. The file has an established tauri event mock used by the
existing listener tests — read its helpers before writing the seam test
and reuse them.

`src-tauri/src/tabs.rs:34-56,108-126` — `Tab::ORDER` (5 variants),
`from_prefix_digit` (1→Agent … 5→News), `wire_label` (`"agent" |
"football" | "music" | "weather" | "news"`; "music", not "media").

`src/components/IconStrip.tsx:20-22`:

```ts
export type Tab = "agent" | "football" | "music" | "weather" | "news";
export const TAB_ORDER: readonly Tab[] = ["agent", "football", "music", "weather", "news"];
```

`src-tauri/src/lib.rs:1952-1964` — `PREFIX_FOLLOWUPS` maps `Digit1..5` to
`PrefixKey::Digit(1..5)`.

The parity-test pattern to model: `src/settings/hookEventParity.test.ts` —
reads sources as text with `readFileSync`, slices a named region between
marker strings, compares extracted sets. Its header comment explains the
trade-off; copy the idiom (including `readText` and `region` helpers or
local equivalents).

`src-tauri/src/settings.rs:334-345` — the rust validator:

```rust
fn is_valid_prefix_shortcut(value: &str) -> bool {
    const PREFIX: &str = "⌃⇧";
    match value.strip_prefix(PREFIX) {
        Some(rest) => {
            let key_chars = rest.chars().count();
            (1..=24).contains(&key_chars) && !rest.chars().any(char::is_whitespace)
        }
        None => false,
    }
}
```

`src/settings/sections/ShortcutsSection.tsx:50-56` — the TS mirror:

```ts
export function isValidPrefixShortcut(raw: string): boolean {
  if (!raw.startsWith(PREFIX_GLYPHS)) {
    return false;
  }
  const rest = Array.from(raw.slice(PREFIX_GLYPHS.length));
  return rest.length >= 1 && rest.length <= 24 && !/\s/.test(rest.join(""));
}
```

Repo conventions: vitest runs with `TZ=UTC` (no time assertions needed
here anyway); tests-as-text parity pins are established practice
(`hookEventParity.test.ts`, `sourceColors.test.ts`); test counts live in
`docs/TESTING_STRATEGY.md` §0 and only there.

## Commands you will need

Web commands from the repo root; rust from `src-tauri/`.

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint gate | `npx biome ci .` | exit 0 |
| Rust tests | `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked` | all pass |

## Scope

**In scope** (the only files you should modify):
- `src/useStatusState.test.ts`, `src/lib/iconPresence.test.ts`,
  `src/App.test.tsx`
- New file `src/tabWireParity.test.ts` (repo-root `src/`, beside the other
  cross-language pins is fine — or `src/lib/`; match where imports are
  simplest)
- `src/settings/sections/ShortcutsSection.tsx` (validator regex only) +
  its existing test file (find with `grep -rln "isValidPrefixShortcut" src/`)
- `src-tauri/src/settings.rs` (test additions only — the shared fixture
  table; the rust validator body does NOT change)
- `docs/TESTING_STRATEGY.md` §0 (counts)

**Out of scope** (do NOT touch):
- `src/useStatusState.ts`, `src/lib/iconPresence.ts`, `src/useTabSelection.ts`
  — production code is correct; this plan adds tests around it.
- `src-tauri/src/tabs.rs`, `src/components/IconStrip.tsx` — parity is
  pinned by reading them, not editing them.
- `icon-strip.css` selectors — the CSS side of the identity list is
  covered by plan 175's geometry pin; don't double-pin.

## Git workflow

- Branch: `advisor/180-tab-wire-test-backfill`
- Commit style: conventional, e.g. `test(tabs): rejection cases, App seam, identity parity pin`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Status-state rejection tests

In `src/useStatusState.test.ts`, add rejection tests in the file's
existing shape (copy the football rejection block's structure):

- payload with `agent` missing entirely → fallback
- `agent.activeSessions: -1` and `agent.activeSessions: 1.5` → fallback
- `news.chargeFraction: 1.4` and `news.chargeFraction: -0.1` → fallback
- `news.chargeCount: -3` → fallback
- `news.isCharged: "yes"` → fallback

In `src/lib/iconPresence.test.ts`, add one test:
`iconPresenceFor(undefined)` returns the documented all-ambient default
(read `iconPresence.ts:55-63` and assert its actual shape).

**Verify**: `npx vitest run useStatusState iconPresence` → all pass.

### Step 2: The App↔tab seam test

In `src/App.test.tsx`, using the file's existing event-mock helpers, add
one describe block:

- render `<App />`; emit the hover/status events the existing tests use to
  reach a hovered-idle state with media present; emit
  `tab-selection-changed` `{ selected: "music" }`; assert the media
  below-block (its testid — see `MediaBelowBlock.tsx`) mounts.
- emit `{ selected: "definitely-not-a-tab" }`; assert the selection is
  ignored (the below-block from the previous step's state does not change /
  with no prior selection the ambient peek remains).

If plans 176/177 have not landed yet, the exact mounted element may
differ — assert what the CURRENT code does and leave a `TODO(plan 176/177)`
comment naming the stronger assertion.

**Verify**: `npx vitest run App` → all pass.

### Step 3: The identity parity pin

New file `src/tabWireParity.test.ts`, modelled on
`src/settings/hookEventParity.test.ts` (text-level, region-scoped):

- read `src-tauri/src/tabs.rs`; extract the `wire_label` match arms'
  string literals in order; assert exact equality with `TAB_ORDER` from
  `src/components/IconStrip.tsx` (import it directly — it's exported).
- extract `Tab::ORDER`'s variant list and assert its order maps 1:1 onto
  the same five tokens (Agent→agent … News→news).
- extract `from_prefix_digit`'s arms and assert digits 1..5 map to the
  five variants in `Tab::ORDER` order.
- read `src-tauri/src/lib.rs`; assert `PREFIX_FOLLOWUPS` contains
  `Digit1` through `Digit5` wired to `Digit(1)`..`Digit(5)`.

Header comment: name the six hand-synced sites and the silent-coercion
failure mode this pin exists to catch (`useTabSelection` validates against
`TAB_ORDER`, so rust-side drift renders as "nothing selected", no error).

**Verify**: `npx vitest run tabWireParity` → passes; temporarily reorder a
local copy? No — do NOT mutate sources to test the test; instead assert
the extraction helpers throw on missing regions (same guard style as
`hookEventParity.test.ts`'s `region` helper).

### Step 4: Unify the whitespace class

In `src/settings/sections/ShortcutsSection.tsx`, replace `/\s/` with an
explicit Unicode `White_Space` class so the TS mirror matches rust's
`char::is_whitespace` exactly:

```ts
const UNICODE_WHITE_SPACE =
  /[\t\n\v\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]/;
```

with a comment naming `settings.rs::is_valid_prefix_shortcut` as the
authoritative twin and the two divergent code points that motivated this
(U+0085 in rust-but-not-`\s`; U+FEFF in `\s`-but-not-rust).

Add the SAME fixture table to both sides' tests: accept `"⌃⇧K"`,
`"⌃⇧K\u{FEFF}"` (FEFF is not White_Space — both sides accept); reject
`"⌃⇧K L"` (space), `"⌃⇧K\u{0085}"` (NEL), `"⌃⇧"` (empty rest), 25-char
rest. Rust side: extend the existing `is_valid_prefix_shortcut` tests in
`settings.rs`'s test module. TS side: extend the existing
`isValidPrefixShortcut` tests.

**Verify**: `npx vitest run Shortcuts` (or the actual test filename) →
pass; from `src-tauri/`, `cargo test --locked prefix_shortcut` → pass.

### Step 5: Full gates + counts

All four commands green. Recount `docs/TESTING_STRATEGY.md` §0 from the
live totals (both suites changed).

## Test plan

The plan IS the test plan — Steps 1-4 enumerate every case. Structural
models: football-rejection block (`useStatusState.test.ts:135` area) for
Step 1; the file's own listener tests for Step 2;
`hookEventParity.test.ts` for Step 3; the existing validator tests both
sides for Step 4.

## Done criteria

- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .` exit 0
- [ ] `cargo test --locked` (from `src-tauri/`) exits 0
- [ ] `src/tabWireParity.test.ts` exists and passes
- [ ] ≥6 new rejection assertions across `useStatusState.test.ts`
- [ ] `grep -n "\\\\s" src/settings/sections/ShortcutsSection.tsx` → the bare `/\s/` test is gone
- [ ] Shared fixture strings appear in both `settings.rs` tests and the TS test
- [ ] `docs/TESTING_STRATEGY.md` §0 recounted (both rows)
- [ ] No files outside the in-scope list modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- Any Step 1 rejection case PASSES validation (i.e. the production
  validator does not reject it) — that is a production bug this plan must
  not silently fix; report which case.
- The parity pin fails at head (the six sites already drifted) — report
  the drift, do not "fix" the rust or TS identity lists yourself.
- `App.test.tsx`'s event mock cannot deliver `tab-selection-changed`
  without new infrastructure — report what's missing instead of building a
  parallel mock.

## Maintenance notes

- Adding a sixth tab now requires touching the six sites AND this pin —
  the pin's failure message should name all six (write it that way).
- The whitespace fixture table is the contract; any future change to
  either validator must run both sides' tables.
- Reviewer focus: Step 4's regex — verify the code-point list against
  Unicode `White_Space` (25 code points; the class above encodes them as
  ranges) rather than trusting this plan by eye.
