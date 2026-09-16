//! Controlled FFmpeg/FFprobe execution for Protocol 0.1 media operations.
//!
//! This crate owns process execution, staging containment and (from SC-P1-02
//! onwards) probe normalization. It never exposes FFmpeg argv or host paths
//! through the protocol: callers pass structured requests and receive
//! structured results.

pub mod process;
pub mod staging;
pub mod toolchain;
