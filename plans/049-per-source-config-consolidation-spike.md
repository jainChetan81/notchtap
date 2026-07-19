# Plan 049 (spike): design a shared shape for per-source config, ending the four-plans-in-a-row copy-paste

> **Executor instructions**: This is a DESIGN SPIKE, not a build plan.
> The deliverable is a design document plus open questions for the
> maintainer — **zero production code changes**. Follow the steps, honor
> the STOP conditions, and when done update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f2cbae6..HEAD -- src-tauri/src/config.rs src-tauri/src/settings.rs src/settings/SettingsApp.tsx`
> Drift doesn't block a spike — but read the drifted regions before
> quoting them in the design doc.

## Status

- **Priority**: P3
- **Effort**: M (coarse — investigation + design doc, no build)
- **Risk**: LOW (docs only)
- **Depends on**: none
- **Category**: direction / tech-debt
- **Planned at**: commit `f2cbae6`, 2026-07-19

## Why this matters

Every push source (espn, rss, manual, cmux, weather) repeats the
identical config shape by hand: an `enabled: bool` field, a
`poll_secs`/`ttl_secs` field, a `priority: Priority` field, a dedicated
`default_X()` free function per field, a manual line in `impl Default
for Config`, and a dedicated assertion block in the config test module.
Four plans in a row (013, 020, 021, 040) each re-derived this same
triad from scratch across three files (`config.rs`, `settings.rs`,
`SettingsApp.tsx`) rather than composing a shared shape — `config.rs`
alone grew to 661 lines almost entirely on this pattern. This is not a
correctness problem today (every instance is well-tested), but it's
"missing abstraction where the same change always requires touching N
files in lockstep" — and the cost compounds with every future source.
This spike investigates what a shared shape would look like and what it
would cost to migrate the five existing sources onto it, so the
maintainer can decide whether to pay that cost now or keep paying the
smaller per-addition tax indefinitely.

**This is deliberately a spike, not a build plan.** The migration this
would require is genuinely large (every existing source's TOML field
names are a back-compat constraint for users' existing
`~/.config/notchtap/config.toml` files) and risk-bearing (it touches the
wire format, the settings IPC contract, and every poller's
config-consumption code simultaneously) — exactly the kind of finding
this repo's own `improve` skill convention scopes as "investigate,
prototype, define the shape, list open questions" rather than "build it
now." A sixth source is not currently planned or requested; this spike
exists so that if one is ever proposed, the maintainer has a concrete
cost/benefit doc to decide against rather than a fifth ad-hoc
copy-paste.

## Current state (grounding — quote-verified at `f2cbae6`)

- The repeated shape, `src-tauri/src/config.rs`:
  - Each source's fields are scattered flat on `Config` — e.g. weather's
    (`config.rs:65-81`, approximate — reconfirm with `rg`):
    `weather_enabled: bool`, `weather_lat: f64`, `weather_lon: f64`,
    `weather_units: Units`, `weather_poll_secs: u64`,
    `weather_rain_threshold_pct: u8`, `weather_rain_lookahead_mins: u16`,
    `weather_temp_hot_c: f64`, `weather_temp_cold_c: f64`,
    `weather_priority: Priority` — 10 fields for one source.
  - A dedicated `default_weather_*()` free function per field
    (`config.rs:209-245`, ten functions: `default_weather_enabled`,
    `default_weather_lat`, `default_weather_lon`,
    `default_weather_units`, `default_weather_poll_secs`,
    `default_weather_rain_threshold_pct`,
    `default_weather_rain_lookahead_mins`, `default_weather_temp_hot_c`,
    `default_weather_temp_cold_c`, `default_weather_priority`) — the
    same pattern repeats for `espn_*`/`rss_*`/`manual_*`/`cmux_*`.
  - A manual line per field inside `impl Default for Config`
    (`config.rs:332-341` for weather alone).
  - A dedicated assertion block in the config test module (e.g.
    `config.rs:433-442` for weather's defaults, `config.rs:621-633` for
    a full weather TOML round-trip).
- The corresponding range-validation block,
  `src-tauri/src/settings.rs:49` (`pub fn validate`), with
  weather-specific checks at `settings.rs:149-166` (rain-threshold
  0-100 bound, hot > cold sanity check) — every source has its own
  hand-written block in the same function.
- The corresponding settings-window UI section,
  `src/settings/SettingsApp.tsx:884` (`function WeatherSection`, wired
  in at `SettingsApp.tsx:1510`) — every source has its own dedicated
  section component following the same declarative-controls-composition
  pattern (`FootballSection`, `NewsSection`, `CmuxSection` are the
  siblings to compare against).
- The four plans that each re-derived this triad from scratch (read
  their `plans/README.md` done-entries for the exact diff shape each
  one produced): 013 (boot-path validation), 020 (`get_default_config`
  single-source-of-truth), 021 (settings save polish), 040 (weather —
  the newest and largest instance, ~9 new fields across all three
  files).
- Config back-compat constraint (must inform any proposed shape):
  `config.rs:373-395`'s existing inheritance-shim precedent for
  preserving `~/.config/notchtap/config.toml` compatibility across a
  schema change — read this before proposing any TOML-shape change, and
  the doc must explicitly address whether a new shared shape can stay
  wire-compatible with today's flat `espn_enabled = true` /
  `weather_poll_secs = 900` style keys, or whether it requires a
  migration path for existing installs.
- `CONTEXT.md`'s **Origin**/**Poller**/**Connector** entries — use these
  terms verbatim; the doc should be explicit about which per-source
  fields are inherent to being a *Poller* (poll_secs) vs. inherent to
  being any *Origin* at all (priority, rotation-order participation) vs.
  connector-relevant (none today — Connectors are configured globally,
  not per-source).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Read-only exploration | `grep`, `Read`, `rg` | — |
| Count the actual duplication | `rg -c "^fn default_" src-tauri/src/config.rs` | a number, cited in the doc (34 at `f2cbae6`; these are private free functions, not `pub` — the `pub fn` variant of this command returns 0) |
| Confirm nothing changed | `git status` at the end | only the new doc + `plans/README.md` row |

## Scope

**In scope** (the only files you may create/modify):
- `docs/design/per-source-config-consolidation.md` (create)
- `plans/README.md` (status row)

**Out of scope — hard rule for this spike**:
- ANY file under `src/`, `src-tauri/`, or config/build files. No
  prototype code in the repo; illustrative snippets (e.g. a proposed
  `SourceConfig` struct shape) live inside the doc only.
- Rewriting `CONTEXT.md`/`ARCHITECTURE.md` — the doc *proposes* whether
  either needs a new term or decision entry; it doesn't make the edit.
- Actually designing or touching a sixth source — this spike is about
  the *shape*, not about adding a new source.

## Git workflow

- Docs-only commit `docs(design): per-source config consolidation spike`
  in repo style. Do NOT push or open a PR unless the operator
  instructed it.

## Steps

### Step 1: Inventory the actual duplication, precisely

Grep every `default_espn_*`/`default_rss_*`/`default_manual_*`/
`default_cmux_*`/`default_weather_*` function in `config.rs` and count
them per source. Read all five sources' field lists on `Config` and
tabulate: which fields every source has in common (candidates:
`enabled`, some rotation-window field, `priority`), and which are
genuinely source-specific (candidates: espn's `leagues: Vec<String>`,
weather's `lat`/`lon`/`units`/threshold fields, rss's `feeds:
Vec<RssFeedConfig>`). A shared shape only helps for the common fields —
the doc must be honest about what fraction of the duplication a shared
shape would actually eliminate versus what's irreducibly
source-specific.

### Step 2: Write the design doc

`docs/design/per-source-config-consolidation.md`, each section with a
**recommendation and at least one rejected alternative with reason**:

1. **Shared shape**: propose a concrete Rust shape (e.g. a
   `SourceConfig { enabled: bool, priority: Priority, poll_secs: Option<u64>,
   ttl_secs: u64 }` sub-struct, embedded per-source, vs. a
   `HashMap<SourceKind, SourceConfig>` replacing the flat fields
   entirely) — trace through what `Config`, `validate()`, and
   `SettingsApp.tsx` would each look like under it.
2. **TOML wire-format compatibility**: does the proposed shape serialize
   to the same flat keys existing `config.toml` files already use
   (`espn_enabled = true`), or does it require a migration/inheritance
   shim like `config.rs:373-395`'s existing precedent? State the answer
   with line references — this is likely the single biggest constraint
   on which shape is viable at all.
3. **Validation**: how `settings.rs`'s `validate()` would apply
   per-source range checks (rain-threshold 0-100, hot>cold, etc.) under
   a shared shape — can source-specific validation logic still live
   next to source-specific fields, or does it need a trait/callback
   per source?
4. **Settings UI**: whether `SettingsApp.tsx`'s five section components
   could share more structure under the new shape, or whether the UI
   layer stays exactly as hand-written as it is today regardless (the
   shape change may only pay off on the rust side).
5. **Migration cost for the five existing sources**: file-by-file blast
   radius estimate (which lines in `config.rs`/`settings.rs`/
   `SettingsApp.tsx`/each poller's config-reading call sites would
   change) — this is the number that determines whether this is worth
   doing at all.
6. **What doesn't get simpler**: be honest about the fields that are
   genuinely source-specific (espn's `leagues`, rss's `feeds`, weather's
   lat/lon/units/thresholds) — a shared shape helps the common 3-4
   fields, not these.
7. **Trigger condition**: under what circumstance is this worth doing —
   e.g. "only if a 6th source is seriously proposed" vs. "worth doing
   proactively regardless." State a recommendation.
8. **Build estimate**: S/M/L with the file list the build would touch,
   contingent on whichever shape Step 2.1 recommends.
9. **Open questions for the maintainer** (e.g.: is TOML back-compat a
   hard requirement or could a one-time config migration on first boot
   be acceptable; is a 6th source actually anticipated in the near
   term).

### Step 3: Sanity-check citations

Every code claim gets a `file:line` valid at the commit read (stamped at
the top of the doc).

**Verify**: `git status` → only the design doc (+ `plans/README.md`
row).

## Test plan

N/A — docs-only spike.

## Done criteria

- [ ] `docs/design/per-source-config-consolidation.md` exists, covers
      all 9 sections, each with recommendation + rejected alternative
- [ ] The doc states the commit it was researched against
- [ ] The TOML-wire-format-compatibility section explicitly answers
      whether the proposed shape breaks existing users' config files
- [ ] No source-code changes (`git status` proof)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- A sixth source already exists or is under active development
  elsewhere in the tree (re-grep `SourceKind`'s variant count before
  starting) — that would mean the "no forcing function yet" premise is
  stale, and the finding should be re-scored as higher priority, not
  quietly written up as a low-urgency spike.
- Understanding the migration cost seems to require actually writing the
  shared-shape code to see what breaks — write the uncertainty into the
  doc instead (e.g. "estimated M-effort, high confidence the TOML
  compatibility question is the crux, but the exact struct shape needs
  a prototype to nail down precisely").

## Maintenance notes

- If approved, the build plan inherits this doc's shape decision and
  TOML-compatibility answer verbatim; if rejected or deferred, record it
  in `plans/README.md`'s rejected/deferred list with the reason (per
  this repo's convention for spike outcomes, e.g. plans 030/031's
  done-entries).
- Whoever eventually builds a sixth source should read this doc first
  regardless of whether the consolidation itself is approved — even if
  the answer is "keep copy-pasting," the doc's inventory of what's
  common vs. source-specific is useful reference for doing the copy
  correctly.
