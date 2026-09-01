//! Domain use cases (SPECS.md §2).
//!
//! Pure operations on domain data. `entry` mutates `&mut Vec<Entry>`;
//! `scan` walks the `Filesystem` port. Persistence stays behind the
//! `RegistryStore` port — callers save after mutating.

pub mod entry;
pub mod scan;
