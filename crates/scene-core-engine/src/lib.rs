//! `scene-core` sidecar CLI: `version --json`, `doctor --json` and the
//! stdin/stdout `run` session.
//!
//! Protocol 0.1 `probe`, `extract_preview` and `extract_audio_pcm` are
//! implemented over the locked FFmpeg/FFprobe toolchain. Wire contracts live
//! in `scene-core-protocol`, process/staging/media work in `scene-core-media`,
//! and this crate owns request validation, event sequencing and the doctor
//! bundle checks.

pub mod cli;
pub mod doctor;
pub mod identity;
pub mod run;
pub mod runner;

pub use cli::run;
