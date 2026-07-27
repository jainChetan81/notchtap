# 144 — Open/Focus Session shortcut `⌃⇧A`

> v7 ticket 12 of 13 (spec §6.3). Filed 2026-07-26.

**What to build:** the global shortcut `⌃⇧A` focuses the
highest-ranked Agent Session's Host application, with a strictly
code-owned activation surface.

- New `agents/focus.rs`: validated focus/open behavior. Supported
  Host bundle IDs + activation strategies owned by notchtap code,
  keyed by a small enum; unknown Host metadata renders as text, never
  actionable.
- Focus tries the known Host app first; an optional provider-native
  deep link only from a code-owned scheme allowlist and only when it
  matches the session's provider; NO `sh -c`, arbitrary executable
  paths, or adapter-provided arguments.
- Failure → logged + quiet status, never a shell fallback.
- Registered like the existing global shortcuts (⌃⇧P pattern);
  overlay stays receive-only — the shortcut lives in Rust.

**Blocked by:** 136 (needs the ranked registry view live; 133's
ordering supplies "highest-ranked").

**Status:** ready-for-agent

- [ ] Unit tests: highest-ranked selection, unknown-Host
      non-actionability, scheme-allowlist rejection of mismatched or
      unlisted deep links.
- [ ] No code path can execute adapter-provided strings.
- [ ] Manual: with a real session under a known Host, `⌃⇧A` activates
      it; with none, quiet no-op status.
- [ ] `cargo test` green.
