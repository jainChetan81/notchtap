# 134 — `POST /agent/events` endpoint

> v7 ticket 2 of 13 (spec §3). Filed 2026-07-26.

**What to build:** the versioned loopback ingestion endpoint. After
this ticket, `curl`-ing a schema-v1 Agent Event at
`127.0.0.1:9789/agent/events` returns `202` and updates the ticket-133
registry; malformed, oversized, duplicate, and stale input is rejected
or idempotently absorbed exactly per spec §3.2.

- New `agents/adapter.rs`: provider-neutral wire parsing of the §3.1
  schema, plus the centralized §3.2 hard-caps table (body 64 KiB, ID
  256 B, summary 500 scalars, names/labels 120, cwd/detail values
  1,024, details 12, capabilities 16, subagents 16). Strings trimmed
  and control-characters stripped before storage.
- Route added to the existing `/notify` listener — same loopback
  binding, Host-header defense, body-limit posture, logging, and
  lifecycle. Prefactor shared request-defense helpers out of
  `notify_handler` rather than duplicating them.
- Status mapping: unknown `schemaVersion`/malformed/unsupported
  runtime/absent identity → `400`; oversized → `413`; accepted → `202`;
  duplicate `eventId` or stale `sequence` → idempotent `202` with no
  registry change; internal failure → `500`.
- Sequence semantics per §3.2: lower-or-equal sequence is stale;
  without sequence, receive order is authoritative and `eventId`
  dedupes; timestamps never override receive order.
- §10 structured log fields (`agent.runtime`, `agent.session_hash`,
  `agent.native_event`, `agent.kind`, `agent.state`, `agent.event_id`);
  raw session IDs and cwd are never logged (hash + basename only).
- No notification mapping yet (ticket 135), no `agent-state` IPC yet
  (ticket 136). `capabilities/default.json` byte-for-byte unchanged.

**Blocked by:** 133 (Agent domain model + registry core).

**Status:** ready-for-agent

- [ ] Integration tests cover every §3.2 status-code row, the
      Host-header defense, and every cap in the table (at, above, and
      trimming behavior).
- [ ] Duplicate-`eventId` and stale-`sequence` tests prove `202` with
      zero registry mutation.
- [ ] Log-hygiene test (or reviewed assertion) proves no raw session
      ID or cwd reaches the log line.
- [ ] Manual: `curl` a permission event → `202`; repeat → `202`
      idempotent; garbage → `400`.
- [ ] `cargo test` green.
