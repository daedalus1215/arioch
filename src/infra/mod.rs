//! Infrastructure adapters: the only place that touches
//! `std::fs` / `std::process` / the TOML index format.

pub mod annotations_store;
pub mod audit_log;
pub mod config;
pub mod fs;
pub mod index_store;
pub mod process;

#[cfg(test)]
pub mod mem;
