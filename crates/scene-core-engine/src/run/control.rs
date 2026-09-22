//! Cancellation, deadline and control-JSONL state for one run.

use scene_core_protocol::{
    ControlMessage, ControlStreamValidator, ProtocolError, StartRequest, parse_control_line,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Shared cancellation and protocol-violation state for one run.
#[derive(Debug, Default, Clone)]
pub struct RunControl {
    cancelled: Arc<AtomicBool>,
    timed_out: Arc<AtomicBool>,
    violation: Arc<Mutex<Option<ProtocolError>>>,
}

impl RunControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn request_timeout(&self) {
        self.timed_out.store(true, Ordering::Relaxed);
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Records a protocol violation (for example a second start or a cancel
    /// with the wrong requestId) and stops the running operation.
    pub fn record_violation(&self, error: ProtocolError) {
        self.cancelled.store(true, Ordering::Relaxed);
        *self.violation.lock().expect("violation lock") = Some(error);
    }

    pub fn violation(&self) -> Option<ProtocolError> {
        self.violation.lock().expect("violation lock").clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn is_timed_out(&self) -> bool {
        self.timed_out.load(Ordering::Relaxed)
    }

    pub fn process_flag(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    pub fn cancelled_flag(&self) -> &AtomicBool {
        &self.cancelled
    }
}

/// Reads control JSONL and applies the protocol rules to it: the first line is
/// the only StartRequest, at most one matching CancelRequest follows, and any
/// other message is a framing violation. Blank lines are ignored.
pub struct ControlSession {
    validator: ControlStreamValidator,
    control: RunControl,
}

impl ControlSession {
    pub fn new(request: &StartRequest, control: RunControl) -> Self {
        let mut validator = ControlStreamValidator::new();
        validator
            .accept(&ControlMessage::Start(Box::new(request.clone())))
            .expect("a freshly parsed StartRequest is accepted");
        Self { validator, control }
    }

    pub fn control(&self) -> &RunControl {
        &self.control
    }

    pub fn handle_line(&mut self, line: &str) {
        if line.trim().is_empty() {
            return;
        }
        match parse_control_line(line) {
            Ok(message) => match self.validator.accept(&message) {
                Ok(()) => {
                    if self.validator.is_cancelled() {
                        self.control.request_cancel();
                    }
                }
                Err(error) => self.control.record_violation(
                    ProtocolError::invalid_request(error.to_string()).with_stage("framing"),
                ),
            },
            Err(error) => self.control.record_violation(*error),
        }
    }
}
