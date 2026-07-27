//! Plan 133/134/135 (v7 tickets 1-3 of 13,
//! `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` §2-§3, §5): the
//! provider-neutral Agent domain model, the authoritative in-memory
//! Agent Registry, the `POST /agent/events` wire parsing + caps table
//! (`adapter.rs`), and the noteworthy-event → existing `Event` mapping
//! (`notification.rs`).
//!
//! This module is the foundation every later v7 ticket builds on:
//! `health.rs` (ticket 143) and `focus.rs` (ticket 144) land on top of
//! the types and the [`registry::AgentRegistry`] built here. As of
//! ticket 135 the registry is wired into `AppState` (`http.rs`) behind
//! [`registry::AgentRegistryHandle`], reachable over HTTP, and a
//! noteworthy event is submitted to the `Engine` exactly like `/notify`
//! — but there is still no Agent Board `agent-state` IPC/event emission
//! (ticket 136) and no config wiring (plan 137): `stale_after`/
//! `terminal_retention` (registry) and every `NotificationPolicy` field
//! (`notification.rs`) are spec-default constructor values for now.
//!
//! The registry lives behind the same application-state boundary as the
//! `Engine` (`engine.rs`), but it is NOT part of the Notification Queue
//! (`queue.rs`) — an Agent Event may update the registry, create a
//! Notification, do both, or do neither, per spec §2.

pub mod adapter;
pub mod board;
// Plan 142 (v7 ticket 10 of 13, spec §6.2 expanded): pure geometry for
// the Agent Board's hover-expanded window frame — see the module's own
// doc for why this deliberately breaks `hover::active_card_rect`'s
// "window frame never changes" invariant, and only for this one case.
pub mod expand;
// Plan 143 (v7 ticket 11 of 13, spec §4.6/§8/§10): per-runtime Adapter
// Health — the pure availability/capability/compatibility-message
// derivation plus the shared `HealthTracker` `http.rs`'s `/agent/events`
// handler updates and `board.rs`/`settings_commands.rs` both read from.
// See its own module doc for the pure/impure split.
pub mod health;
// Plan 144 (v7 ticket 12 of 13, spec §6.3): the `⌃⇧A` Open/Focus Session
// shortcut's pure decision logic + activation call. Lands on top of
// `model::AgentState`/`registry::AgentRegistryHandle::ordered_states`
// built above — see its own module doc for the security invariants.
pub mod focus;
pub mod model;
pub mod notification;
// Plan 138 (v7 ticket 6 of 13, spec §4.1/§4.2): the `notchtap-agent`
// binary's own logic — pure per-provider stdin parsers, the schema-v1
// wire builder, delivery (fail-open, ≤750ms), and the bounded adapter
// diagnostic log. Kept as its own submodule tree (rather than folding
// into `adapter.rs`/`model.rs`) because it's the one part of `agents/`
// a *separate binary crate* needs to reach — see `../lib.rs`'s doc on
// why this module became reachable outside this crate.
pub mod providers;
pub mod registry;
