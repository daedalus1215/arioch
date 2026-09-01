//! Application layer: thin command handlers between the CLI parse and the
//! domain. No TUI; no persistence details (that is the `RegistryStore`
//! port).

pub mod cli;
