//! Provider-neutral Agent domain model, the authoritative in-memory Agent Registry, `POST
//! /agent/events` wire parsing + caps table (`adapter.rs`).

pub mod adapter;
pub mod board;
// Deliberately breaks `hover::active_card_rect`'s "window frame never changes" invariant, for this
// one case only — see its module doc.
pub mod expand;
pub mod focus;
pub mod health;
pub mod model;
pub mod notification;
pub mod providers;
pub mod registry;
