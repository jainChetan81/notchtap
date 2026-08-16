//! Provider-neutral Agent domain model, the authoritative in-memory
//! Agent Registry, `POST /agent/events` wire parsing + caps table
//! (`adapter.rs`), and the noteworthy-event → `Event` mapping
//! (`notification.rs`).
//!
//! The registry lives behind the same application-state boundary as the
//! `Engine` (`engine.rs`) but is NOT part of the Notification Queue
//! (`queue.rs`) — an Agent Event may update the registry, create a
//! Notification, do both, or do neither.

pub mod adapter;
pub mod board;
// Deliberately breaks `hover::active_card_rect`'s "window frame never
// changes" invariant, for this one case only — see its module doc.
pub mod expand;
// Shared `HealthTracker`: updated by `http.rs`'s `/agent/events`
// handler, read by `board.rs` and `settings_commands.rs`.
pub mod focus;
pub mod health;
pub mod model;
pub mod notification;
// Its own submodule tree because it's the one part of `agents/` a
// separate binary crate reaches — see `../lib.rs`.
pub mod providers;
pub mod registry;
