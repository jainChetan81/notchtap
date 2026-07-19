# Plan 051 (spike): let Manual/Cmux pushes carry a `link`, unlocking ⌃⇧O outside News

> **Executor instructions**: This is a DESIGN SPIKE, not a build plan.
> The deliverable is a design document plus open questions for the
> maintainer — **zero production code changes**. Follow the steps, honor
> the STOP conditions, and when done update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f2cbae6..HEAD -- src-tauri/src/http.rs src-tauri/src/event.rs src-tauri/src/lib.rs notchtap`
> Drift doesn't block a spike — but read the drifted regions before
> quoting them in the design doc.

## Status

- **Priority**: P3
- **Effort**: S (coarse — the eventual build looks small; the spike
  itself is quick)
- **Risk**: LOW (docs only, for this spike)
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `f2cbae6`, 2026-07-19

## Why this matters

The ⌃⇧O ("open story") global hotkey opens the currently-visible
notification's link in the default browser. Mechanically, it is already
fully source-agnostic: `open_current_story` (`src-tauri/src/lib.rs:720`)
reads `engine.read_blocking(|q| q.current_link())`
(`src-tauri/src/queue.rs:409-413`, which just reads
`item.event.meta.link`), validates it through the tested
`openable_http_url` gate (`lib.rs:711-717`, http(s)-only, returns the
normalized serialization), and shells out to `open -u`. Nothing in that
path checks `EventType` or `Origin` — the only reason this is "news
only" today (as `README.md`'s hotkey table documents it) is that the RSS
poller is currently the only code that ever sets `EventMeta.link`.

But `src-tauri/src/http.rs:181-183` states this as a **deliberate,
already-decided rule**, not an oversight to casually reverse:

```rust
// plan 035: subtitle/details are the only meta a `/notify` caller may
// set (source/category/published/link stay poller-only); both are
// sanitized/capped here — this is the trust boundary for hook input.
```

This spike exists because the mechanism/asymmetry is real and cheap to
close (`event.rs`'s wire-schema rule is exactly the "closed door" pattern
the maintainer already deliberately used for `topic`/`rotation`/`origin`
— see `event.rs:15-18`), but it is explicitly opening something plan 035
closed on purpose. That distinction is why this is a spike (surface the
trade-off, get the decision) rather than a direct build, mirroring how
this repo already runs plans 030/031/053 for exactly this class of
"the machinery makes it cheap, but should we?" question.

## Current state (grounding — quote-verified at `f2cbae6`)

- `src-tauri/src/lib.rs:711-717` — the source-agnostic validator:

  ```rust
  fn openable_http_url(raw: &str) -> Option<String> {
      let parsed = reqwest::Url::parse(raw).ok()?;
      match parsed.scheme() {
          "http" | "https" => Some(parsed.to_string()),
          _ => None,
      }
  }
  ```

- `src-tauri/src/lib.rs:719-749` — the source-agnostic hotkey handler,
  `open_current_story`, reads `engine.read_blocking(|q|
  q.current_link()...)`, validates via the function above, then spawns
  `open -u <normalized>` with the child reaped off-thread. Nothing here
  checks event origin/type.

- `src-tauri/src/queue.rs:409-413`:

  ```rust
  pub fn current_link(&self) -> Option<&str> {
      self.visible
          .as_ref()
          .and_then(|item| item.event.meta.link.as_deref())
  }
  ```

- `src-tauri/src/event.rs:152-156` (approximate — reconfirm with `rg`) —
  `EventMeta.link: Option<String>` already exists as a field; only the
  RSS poller populates it today.

- `src-tauri/src/http.rs:64-85` — `NotifyRequest`, the `/notify` wire
  schema (extended by plan 035 with `subtitle`/`details`):

  ```rust
  #[derive(Deserialize)]
  struct NotifyRequest {
      title: Option<String>,
      body: Option<String>,
      priority: Option<Priority>,
      #[serde(default)]
      signal: EventSignal,
      source: Option<RequestSource>,
      subtitle: Option<String>,
      details: Option<Vec<DetailItem>>,
  }
  ```

  No `link` field. `http.rs:181-188`'s `EventMeta` construction
  explicitly notes "source/category/published/link stay poller-only."

- The plan-035 precedent for adding a new sanitized, capped wire field
  (the pattern any build here would follow): `SUBTITLE_MAX_CHARS`,
  `sanitize_subtitle` (`http.rs:92,111-115`), and the CLI's `--subtitle`/
  `--detail` flags (`notchtap` script, repo root, lines 4-9, 58-75,
  108-134) — a `--link` flag would follow the same
  parse-flag→conditionally-merge-into-JSON-payload shape.

- `README.md`'s hotkey table (search for "⌃⇧O") currently documents the
  hotkey as opening "the story/link for the visible item (news only)" —
  this phrasing itself is evidence of the current closed-door state, not
  a bug to silently fix; any build here updates this doc line as part of
  its own scope.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Read-only exploration | `grep`, `Read`, `rg` | — |
| Confirm today's only `meta.link` producer | `grep -rn "link: link\.map\|link: Some(" src-tauri/src/*.rs` | hits in `rss_poller.rs:333` (production, inside the `EventMeta { ... }` struct literal in `diff_feed_events`) plus `rss_poller.rs:804` and `event.rs:331` (both inside `#[cfg(test)] mod tests`) — confirm `rss_poller.rs:333` is the only non-test hit. (The plan's original pattern, `"meta.link\s*=\|\.link:"`, does not match this struct-literal field syntax and instead only catches a `queue.rs` test fixture's direct-field-mutation `story.meta.link = ...` — it does not find the real producer at all.) |
| Confirm nothing changed | `git status` at the end | only the new doc + `plans/README.md` row |

## Scope

**In scope** (the only files you may create/modify):
- `docs/design/manual-cmux-link-field.md` (create)
- `plans/README.md` (status row)

**Out of scope — hard rule for this spike**:
- ANY file under `src/`, `src-tauri/`, or `notchtap` (the CLI script).
  No prototype code in the repo; illustrative snippets live inside the
  doc.
- Rewriting `docs/V3_6_TECHNICAL_SPEC.md`/`README.md` — the doc
  *proposes* the wording change either would need; it doesn't make the
  edit.

## Git workflow

- Docs-only commit `docs(design): manual/cmux link field spike` in
  repo style. Do NOT push or open a PR unless the operator instructed
  it.

## Steps

### Step 1: Confirm the closed-door rule and its stated reason

Read `http.rs:181-183`'s comment and plan 035's done-entry in
`plans/README.md` in full for why `link` was deliberately left off the
`/notify` wire schema when `subtitle`/`details` were added. If a reason
beyond "not needed yet" is recorded, quote it in the doc; if it really
was just scope discipline (not a security/design objection), say so —
that changes how strongly the doc should recommend reopening it.

### Step 2: Write the design doc

`docs/design/manual-cmux-link-field.md`, each section with a
**recommendation and at least one rejected alternative with reason**:

1. **Wire field**: add `link: Option<String>` to `NotifyRequest`,
   sanitized through a new `sanitize_link` mirroring `sanitize_subtitle`
   (validate via the existing `openable_http_url` at accept-time, not
   just at open-time — decide whether an invalid/non-http(s) link
   should be silently dropped, like an empty subtitle collapsing to
   `None`, or rejected with a 400; recommend one with reasoning, noting
   the asymmetry that today's RSS-poller-set links are never validated
   at ingest, only at open-time).
2. **CLI surface**: a `--link <url>` flag on the `notchtap` script,
   following the exact `--subtitle`/`--detail` precedent (conditionally
   merged into the JSON payload only when set).
3. **Hotkey/doc update**: `README.md`'s hotkey table wording ("news
   only" → drop that qualifier or rephrase); confirm no other doc
   (`docs/V3_6_TECHNICAL_SPEC.md`, `CONTEXT.md`) describes ⌃⇧O as
   News-specific in a way that would also need updating.
4. **Manifest/UI affordance**: today's manifest (`src/components/Manifest.tsx`)
   renders subtitle/detail cells but no explicit "has a link" indicator
   — the hotkey works invisibly today for News cards too. Recommend
   whether a Manual/Cmux card with a link should get any visual
   affordance (e.g. a small icon) or whether staying invisible-until-pressed
   (today's News behavior) is fine to keep for consistency. Flag the
   scope-creep risk explicitly: once Manual/Cmux can carry a link,
   expect a follow-up request for a clickable affordance in the card
   itself — recommend NOT building that preemptively.
5. **Security/trust boundary**: `link` becomes a second untrusted-string
   field alongside `details[]/subtitle` (same `SUBTITLE_MAX_CHARS`-style
   cap discipline, or a dedicated URL-length cap) — the value is already
   protected by `openable_http_url`'s http(s)-only gate at open-time
   regardless of source, so this is a low-incremental-risk addition;
   state this explicitly rather than assuming it.
6. **cmux-specific consideration**: cmux relay pushes (via the `--source
   cmux` CLI path) already carry a `context.cwd`/project detail per plan
   035 — evaluate whether cmux's own hook script
   (`hooks/notchtap-cmux-hook.sh`) has a natural link to attach (e.g.
   a deep link back to the originating session/terminal) or whether this
   is purely a manual/scripted-caller feature for now. Don't invent a
   cmux-side capability that doesn't exist — read the hook script first.
7. **Test strategy**: name the concrete new cases (`sanitize_link`
   unit tests mirroring `sanitize_subtitle`'s, an http.rs integration
   test posting a `link` field and confirming it reaches `meta.link`,
   an `openable_http_url`-rejection case for a non-http(s) `link`).
8. **Build estimate**: S, with the exact file list (`http.rs`,
   `notchtap` CLI script, `README.md`'s hotkey table, possibly
   `docs/V3_6_TECHNICAL_SPEC.md`).
9. **Open questions for the maintainer** (e.g.: is this actually wanted,
   or was plan 035's "link stays poller-only" line a considered
   decision the maintainer would rather leave standing; should invalid
   links 400 or silently drop).

### Step 3: Sanity-check citations

Every code claim gets a `file:line` valid at the commit read (stamped at
the top of the doc).

**Verify**: `git status` → only the design doc (+ `plans/README.md`
row).

## Test plan

N/A — docs-only spike.

## Done criteria

- [ ] `docs/design/manual-cmux-link-field.md` exists, covers all 9
      sections, each with recommendation + rejected alternative
- [ ] The doc states the commit it was researched against
- [ ] The doc explicitly quotes and engages with plan 035's "link stays
      poller-only" comment rather than treating the current state as an
      unexamined oversight
- [ ] No source-code changes (`git status` proof)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- `plans/README.md`'s plan-035 done-entry (or any other doc) reveals a
  specific rejected rationale for keeping `link` poller-only beyond
  general scope discipline (e.g. a stated security concern) — that
  changes this from "cheap direction idea" to "a decision already made
  for a reason," and the doc should say so plainly rather than
  recommending reopening it as if it were simply unconsidered.
- `EventMeta.link` or `current_link()`'s shape has changed since
  `f2cbae6` in a way that changes the mechanism's source-agnosticism
  claim — re-verify before writing the doc.

## Maintenance notes

- If approved, the build plan should follow plan 035's exact
  sanitize-and-cap pattern for the new field, and must update
  `README.md`'s hotkey table wording as part of its own scope (not leave
  it stale the way this audit found several other doc claims already
  are — see plan 046).
- This spike and plan 053 (generalizing Topic supersession to
  Manual/Cmux) both touch the `/notify` wire schema's currently-closed
  fields (`link` here, `topic` there) — whoever builds either first
  should note in their own plan/PR that the other closed-door field
  still exists, so a reviewer doesn't assume "we already opened this
  up" from one when only the other landed.
