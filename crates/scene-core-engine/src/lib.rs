//! `scene-core` sidecar CLI: `version --json` and `doctor --json`.
//!
//! The engine has no media runtime yet; these commands freeze the identity and
//! bundle-integrity contracts described by Protocol 0.1.

pub mod cli;
pub mod doctor;
pub mod identity;
pub mod runner;

pub use cli::run;
