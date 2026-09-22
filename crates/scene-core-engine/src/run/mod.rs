//! Host-facing `run` session: validation, event sequence, terminal state and
//! exit codes. Media work is injected through [`MediaBackend`].
//!
//! The implementation is split into private submodules: control state
//! ([`RunControl`], [`ControlSession`]), media adapters and the request/event
//! state machine.

use std::io;
use std::path::Path;

mod backend;
mod control;
mod session;

/// Removes a published or temporary file. A concurrent successful cleanup is
/// equivalent to success; every other I/O failure must reach the caller.
pub(super) fn remove_file_if_exists(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub use backend::{
    AudioPcmOutcome, FfmpegBackend, MediaBackend, MediaFailure, PreviewOutcome, UnavailableBackend,
};
pub use control::{ControlSession, RunControl};
pub use session::{
    Echo, VerifiedInput, cancelled_error, event_for, invalid_request_error, run_session,
    timeout_error, unsupported_protocol_error, validate_request,
};
