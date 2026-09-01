//! Pure domain layer.
//!
//! Hard rule (SPECS.md §2): no ratatui, no crossterm, no `std::fs`, no
//! `std::process`, no shellexpand — everything I/O-shaped is a port trait
//! (Phase 2) implemented in `infra/` and bound at the composition root.
//! Enforced by `tests/boundaries.rs` (Phase 8).

pub mod entity;
pub mod knowledge;
pub mod rules;
pub mod value;
