# Design spike: let Manual/Cmux pushes carry a `link`, unlocking ⌃⇧O outside News

> **Status**: design spike (plan 051), zero production code changes.
> Researched against commit `f2cbae6` (the plan's planned-at commit,
> 2026-07-19). Every `file:line` citation below was re-verified by
> reading the file directly in this working copy; line numbers match
> `f2cbae6` exactly unless noted.

## Why this matters

The ⌃⇧O ("open story") global hotkey opens the currently-visible
notification's link in the default browser. Mechanically it is already
**fully source-agnostic**: `open_current_story`
(`src-tauri/src/lib.rs:719-749`) reads
`engine.read_blocking(|q| q.current_link().map(str::to_string))`
(`lib.rs:721`), where `current_link` (`src-tauri/src/queue.rs:409-413`)
is just:

```rust
pub fn current_link(&self) -> Option<&str> {
    self.visible
        .as_ref()
        .and_then(|item| item.event.meta.link.as_deref())
}
```

The value is then validated through `openable_http_url`
(`lib.rs:711-717`) — a full `reqwest::Url::parse` plus an
http(s)-only scheme match that returns the parser's own normalized
serialization — and handed to `open -u <normalized>` with the child
reaped off-thread (`lib.rs:731-744`). Nothing in that path checks
`EventType` or `Origin`. The only reason ⌃⇧O is "news only" today (as
`README.md:40`'s hotkey table documents it) is that the RSS poller is
the only code that ever sets `EventMeta.link`: `grep` for the
struct-literal producers finds `src-tauri/src/rss_poller.rs:333`
(`link: link.map(str::to_string)`, inside `diff_feed_events`'s
`EventMeta { … }` literal) as the sole non-test hit — the other two
(`rss_poller.rs:804`, `event.rs:331`) are both inside `#[cfg(test)]`
code.

But `src-tauri/src/http.rs:181-183` states the current closure as a
**deliberate rule**, not an oversight:

```rust
// plan 035: subtitle/details are the only meta a `/notify` caller may
// set (source/category/published/link stay poller-only); both are
// sanitized/capped here — this is the trust boundary for hook input.
```

This spike exists to surface that trade-off and get a decision, not to
silently reverse it.

## Step-1 finding: was "link stays poller-only" a considered decision?

**No recorded rationale beyond scope discipline was found.** The
evidence, checked in full:

- The plan-035 done entry in `plans/README.md` (row 035) describes what
  shipped (`subtitle`/`details[]`, CLI flags, hooks, manifest Layout A)
  and says nothing about why `link` was excluded.
- Plan 035's own plan file (`plans/035-rich-relay-manifest(done).md`)
  mentions `link` exactly once, at line 84, purely descriptively
  ("`EventMeta` …: `source/category/published_at_ms/link`, all
  optional, presentation-only") — no rejection rationale, no security
  concern, no design objection.
- The `http.rs:181-183` comment itself asserts the rule but gives no
  reason for it.
- The strongest nearby *design* statement is the `EventMeta` doc
  comment (`src-tauri/src/event.rs:145-149`): "the rss poller populates
  source/category/published/link, and `/notify` callers populate
  subtitle/details" — again descriptive of the split, not argued.

Contrast with the fields that *do* have an argued closure: `origin` is
"Always server-assigned, never accepted from the `/notify` wire (same
rule as `rotation`/`topic`)" (`event.rs:15-19`), and `RequestSource`
is deliberately a closed single-variant set because
"`Football`/`News` must never be wire-claimable, since only the
ESPN/RSS pollers may legitimately produce those" (`http.rs:52-57`).
Those closures protect *identity/attribution* — a caller must not be
able to impersonate a source. `link` is not in that category: it is a
presentation-only field (`event.rs:148-149`: "Presentation-only —
never consulted by queue/rotation/priority logic") whose consumer is
already gated at open-time regardless of which source set it.

**Conclusion**: plan 035's "link stays poller-only" reads as scope
discipline (ship `subtitle`/`details`, change nothing else), not a
stated security or design objection. No STOP condition fires. This
means the doc can recommend reopening — but the maintainer should still
confirm the closure wasn't deliberate for a reason that was never
written down (§9, question 1).

## 1. Wire field

**Recommendation**: add `link: Option<String>` to `NotifyRequest`
(`src-tauri/src/http.rs:64-85`), populated into `EventMeta` through a
new `sanitize_link` that mirrors `sanitize_subtitle`'s shape
(`http.rs:111-115`) but validates instead of merely truncating:

```rust
// illustrative only — NOT landed code
fn sanitize_link(link: Option<String>) -> Option<String> {
    link.and_then(|raw| openable_http_url_for_ingest(&raw))
}
```

i.e. an absent or empty link collapses to `None`; a present link is
parsed and kept only if it is a well-formed http(s) URL (the same rule
`openable_http_url` at `lib.rs:711-717` applies at open-time), stored
in its normalized serialization; anything else collapses to `None`.

**Invalid input should be silently dropped (→ `None`), not rejected
with a 400.** Reasoning:

- The established precedent for optional, presentation-only wire
  fields is coerce-and-continue: an empty subtitle collapses to `None`
  (`http.rs:109-115`), detail pairs with empty labels are dropped and
  the rest capped (`http.rs:117-131`). A notification whose *only*
  defect is a bad link is still a notification worth showing; 400-ing
  it would punish the caller for an auxiliary field.
- 400 rejection does exist in this handler, but for *malformed
  envelope/identity* input — bad JSON (`http.rs:334-339`), missing
  title/body (`http.rs:354-366`), unknown `source`/`signal` strings
  (`http.rs:628-649`, `714-726`) — where silently coercing would
  mis-attribute the event. A bad link mis-attributes nothing; worst
  case, ⌃⇧O no-ops exactly as it does today when no link is present
  (`lib.rs:721-724` debug-logs and returns).
- Note the asymmetry honestly: today's RSS-poller links are **never
  validated at ingest** (`rss_poller.rs:333` stores the feed's link
  verbatim); they are only validated at open-time by
  `openable_http_url`. Validating wire-submitted links at accept-time
  makes `/notify`-sourced links *more* strictly checked than poller
  ones. That is acceptable (the open-time gate stays as the universal
  backstop) but worth stating.

**Rejected alternative 1 — reject invalid links with 400.** Consistent
with the unknown-`source`/`signal` precedent, but wrong category: those
rejections protect attribution; a link is presentation-only, and the
open-time gate already neutralizes a bad value. Rejecting would also
break the hook scripts' fail-safe philosophy (fire-and-forget,
exit-0 — `hooks/notchtap-cmux-hook.sh:41-42`) by turning a cosmetic
defect into a lost notification.

**Rejected alternative 2 — accept any string, validate only at
open-time (mirror the RSS path exactly).** Cheapest diff, but it
stores unvalidated untrusted input in `EventMeta` and pushes it to the
frontend on every `slot-state` emit (`SlotState::Showing.link`,
`event.rs:194`; validated as null-or-string by
`src/useSlotState.ts:39,97`). Accept-time validation keeps the stored
and emitted value canonical — what was validated is what is stored is
what is opened.

## 2. CLI surface

**Recommendation**: add a `--link <url>` flag to the `notchtap` script,
following the exact `--subtitle` precedent: parse in the `case` loop
(alongside `--subtitle` at `notchtap:58-60`), then conditionally merge
into the JSON payload only when set — the same
`+ (if $link != "" then {link: $link} else {} end)` shape as the
existing merges at `notchtap:125-134`. Update the usage line
(`notchtap:32`) and the header comment (`notchtap:4-9`) in the same
change. No client-side URL validation: the server is the trust
boundary (`http.rs:181-183`), and the CLI already defers all
sanitization there — the details cap at `notchtap:44-48` is a courtesy,
never the guarantee.

**Rejected alternative — no CLI flag, wire field only.** A raw-curl
caller could already use the field, but every real caller goes through
this script (the hooks at `hooks/notchtap-cmux-hook.sh:37-41` invoke
`notchtap`, not curl); shipping the wire field without the flag ships
an unreachable feature for the exact callers it targets.

## 3. Hotkey/doc updates

**Recommendation**: the build must update `README.md:40`'s hotkey table
row — `| ⌃⇧O | open the story/link for the visible item (news only) |`
— dropping the "(news only)" qualifier, e.g. "open the link for the
visible item, when it has one". It must **also** update
`docs/V3_6_TECHNICAL_SPEC.md:275-277`, which currently reads "the v5.1
`link: Option<String>` field — the target for the ⌃⇧O open-story
hotkey on news items; absent for non-news sources" — that "absent for
non-news sources" clause becomes false the moment this lands.

`CONTEXT.md` needs no change: it never mentions ⌃⇧O (grep finds only
the generic expand/auto-retract hotkey prose at `CONTEXT.md:74-75`).
`docs/IMPLEMENTATION_PLAN.md:686-709` (§4.6.2, the v5.1 landed log)
describes ⌃⇧O as news-scoped ("wait for a headline, ⌃⇧O opens the
article… a cmux card does nothing") but that file is a dated history
log, not living documentation — leave it.

**Rejected alternative — leave the docs stale and fix them in a later
docs sweep.** This is precisely the drift plan 046's audit flagged
elsewhere; the doc wording change is one line per file and belongs in
the build's own scope, per this spike plan's maintenance notes.

## 4. Manifest/UI affordance

**Correction to the spike plan's premise first**: the plan claims the
manifest has "no explicit 'has a link' indicator" and the hotkey is
invisible today. That is not the current state. `Manifest.tsx` takes a
`hasLink` prop (`src/components/Manifest.tsx:16,26`) fed by
`StatusRailCard.tsx:170` (`hasLink={slot.link !== null}`), and renders
the hint "⌃⇧O read · ⌃⇧N collapse" instead of "⌃⇧N collapse" when a
link is present — in **both** the news branch (`Manifest.tsx:73`) and
the generic branch (`Manifest.tsx:88`). Because the generic branch
already reacts to `slot.link`, a Manual/Cmux card carrying a link would
get the existing "⌃⇧O read" hint **for free, with zero frontend
changes**.

**Recommendation**: build nothing new in the UI. The existing
`hasLink`-driven hint is source-agnostic already, so the affordance
question answers itself. Explicitly do **not** preemptively build a
clickable affordance (icon, button, hyperlink) in the card itself:
expect that follow-up request the moment Manual/Cmux cards can carry
links, and let it arrive as its own plan with its own design
(click handling in a borderless always-on-top overlay is a real
interaction-design question, not a rider on this change).

**Rejected alternative — add a distinct link icon/badge for
Manual/Cmux cards.** Unnecessary (the hint already appears), adds
visual language the fixed 500×300 window must budget for, and violates
Layout A's "no new visual language" discipline from plan 035.

## 5. Security/trust boundary

**Recommendation**: treat `link` as a second untrusted-string field
alongside `subtitle`/`details`, with the same cap discipline. Give it a
dedicated `LINK_MAX_CHARS` cap rather than reusing
`SUBTITLE_MAX_CHARS` (120, `http.rs:92`): URLs legitimately exceed 120
chars (tracking-laden feed URLs routinely run 300+); 1024 is a
sensible cap — far beyond any real URL, far inside the 64 KiB request
body limit (`http.rs:136`). State the risk plainly: the incremental
risk is **low**, because the value's only sink is `open -u` behind
`openable_http_url`'s http(s)-only full-parse gate at open-time
(`lib.rs:706-717`), which runs regardless of which source set the
link. A `javascript:`, `file:`, or `httpx://` value can never reach
`open` (the doc comment at `lib.rs:706-709` already explains why the
parse is never a prefix check). The loopback-only bind
(`http.rs:140-145`) and 64 KiB body cap bound who can submit and how
much.

**Rejected alternative — treat link as needing no cap because the
open-time gate exists.** The gate bounds *scheme*, not *length*; an
unbounded string still lands in `EventMeta`, the queue, and every
`slot-state` emit. Every other untrusted field in this handler is
capped (`http.rs:87-95`); a link should be too.

## 6. cmux-specific consideration

**Recommendation**: ship this as a manual/scripted-caller feature; do
**not** wire a link into the cmux hook in this build. Reading
`hooks/notchtap-cmux-hook.sh` first, as instructed: the hook's stdin
payload is documented at lines 12-13 as `{ "notification": {title,
subtitle, body, ...}, "context": {cwd, ...}, "effects": {...} }`, and
the script consumes exactly `notification.title`, `.body`, `.subtitle`
(`:27-29`) and `context.cwd` (`:30`), which it relays as a
`--detail "Project=$cwd"` pair (`:39`). There is **no URL anywhere in
cmux's documented hook payload**, and `context.cwd` is a filesystem
path — even if naively mapped to `file://$cwd`, it would be rejected by
the http(s)-only `openable_http_url` gate anyway. There is no natural
link for the cmux relay to attach today, and this spike does not invent
one. If cmux later grows a session/terminal deep-link, that is a
separate, small change to the hook script alone.

**Rejected alternative — synthesize a `file://` link from
`context.cwd` so cmux cards "open the project folder".** Dead on
arrival at the open-time gate (non-http(s)), and opening Finder windows
from a notification hotkey is a different feature wearing this one's
clothes.

## 7. Test strategy

Concrete new cases, all following existing shapes:

1. `sanitize_link` unit tests mirroring `sanitize_subtitle_empties_and_caps`
   (`http.rs:882-895`): `None` → `None`; empty string → `None`; valid
   http(s) URL → kept, normalized; over-cap URL → truncated or dropped
   (decide with the cap rule in §5; dropping is cleaner than a
   truncated, possibly-broken URL).
2. An `openable_http_url`-rejection case per scheme: `ftp://`,
   `file://`, `javascript:` → `None` (mirrors the lib.rs gate's own
   test discipline, applied at ingest).
3. An http.rs integration test posting a `link` field and confirming it
   reaches `meta.link`, mirroring
   `notify_round_trips_subtitle_and_details_into_slot_state`
   (`http.rs:936-964`).
4. A back-compat test mirroring
   `notify_without_subtitle_or_details_leaves_them_empty`
   (`http.rs:987+`): an old payload without `link` yields
   `meta.link == None`, byte-identical behavior.
5. No frontend tests needed: the `hasLink` hint path
   (`Manifest.tsx:73,88`) is already generic-branch covered since
   plan 035.

**Rejected alternative — extend the RSS poller tests.** Untouched by
this change; its `link` coverage (`rss_poller.rs:798-809`) already
pins the poller path.

## 8. Build estimate

**S.** Exact file list:

- `src-tauri/src/http.rs` — `NotifyRequest` field, `sanitize_link` +
  `LINK_MAX_CHARS`, meta construction, tests (~1 focused change +
  tests, same shape as plan 035's).
- `src-tauri/src/lib.rs` — at most moving/exposing the URL validation
  so `http.rs` can share it with `open_current_story`; no behavior
  change to the hotkey path.
- `notchtap` (CLI script) — `--link` flag + payload merge + usage text.
- `README.md:40` — hotkey table wording.
- `docs/V3_6_TECHNICAL_SPEC.md:275-277` — drop the "absent for
  non-news sources" clause.
- No changes needed in `src/` (frontend), `hooks/`, `event.rs`
  (`EventMeta.link` and `SlotState::Showing.link` already exist at
  `event.rs:156` and `event.rs:194`), `queue.rs`, or `useSlotState.ts`.

**Rejected alternative — fold this into plan 053's build** (053
generalizes Topic supersession to Manual/Cmux, the other closed-door
`/notify` field). They are independent fields with independent
decisions; per the spike plan's maintenance notes, whichever builds
first should simply note the other's closed-door field still exists.

## 9. Open questions for the maintainer

1. **Is this actually wanted?** Plan 035's "link stays poller-only"
   line has no written rationale (Step-1 finding above) — but was it a
   considered decision the maintainer would rather leave standing
   anyway? E.g. "notifications I push myself never need to open
   anything" is a legitimate product answer that ends this spike here.
2. **Invalid-link handling**: silently drop to `None` (recommended,
   §1) or 400? If the maintainer sees the unknown-`source`/`signal`
   400s as the truer precedent, the build flips one branch.
3. **The cap**: dedicated `LINK_MAX_CHARS` (recommended 1024, §5) —
   and should an over-cap link be dropped or truncated? Dropping is
   recommended (a truncated URL is a broken URL stored).
4. **Doc-scope check**: is `docs/IMPLEMENTATION_PLAN.md` §4.6.2 truly a
   frozen history log (this doc assumes yes), or does the maintainer
   amend landed-plan logs when behavior changes?
