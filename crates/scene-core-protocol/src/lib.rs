//! `scene-core` Protocol 0.1: strict Rust DTOs, pure validation functions and
//! exact media-time conversion.
//!
//! The crate is the structural source of truth for the Host-facing JSON/JSONL
//! contract. It performs no media execution, no cache access and no I/O:
//! `probe` and `extract_preview` results, normalized media, Manifest and
//! Scene/Shot seam DTOs are frozen here so Schema, fixtures and the engine can
//! share one definition.

pub mod canonical;
pub mod cli;
pub mod control;
pub mod error;
pub mod event;
pub mod execution;
pub mod framing;
pub mod identity;
pub mod input;
pub mod manifest;
pub mod media;
pub mod operation;
pub mod package;
pub mod result;
pub mod segment;
pub mod time;
pub mod values;

pub use canonical::{CanonicalJsonError, canonical_sha256, to_canonical_json};
pub use cli::{
    CliErrorOutput, DoctorCheck, DoctorCheckCode, DoctorOutput, DoctorStatus, VersionOutput,
};
pub use control::{
    CancelMessageType, CancelRequest, ControlMessage, ControlStreamValidator, DEFAULT_DEADLINE_MS,
    MAX_DEADLINE_MS, StartMessageType, StartRequest, parse_control_line,
};
pub use error::{ErrorCode, ProtocolError, ProtocolResult, TerminalKind, ValidationError};
pub use event::{EventEnvelope, EventMessageType, EventStreamValidator, EventType};
pub use execution::{ExecutionContext, ExecutionScope};
pub use identity::{DerivationDescriptor, DerivationIdentity, EngineIdentity, ToolchainIdentity};
pub use input::{
    InputDescriptor, InputSetDescriptor, MAX_INPUT_BYTE_SIZE, StagedSourceFacts,
    verify_input_fingerprint, verify_staged_source,
};
pub use manifest::{Artifact, ArtifactKind, ArtifactManifest, ArtifactRole};
pub use media::{ContainerInfo, MediaStream, NormalizedMedia, StreamKind};
pub use operation::{Operation, OperationOptions};
pub use package::{
    DistributionProfile, PackageManifest, PackagedEngine, PackagedFile, PackagedTool,
};
pub use result::{ExtractPreviewResult, OperationResult, ProbeResult, ResourceUsage};
pub use segment::{
    SegmentProvenance, TemporalSegment, TemporalSegmentKind, validate_temporal_segments,
};
pub use time::{
    ExactTime, Rational, TimeError, container_start_ms, round_public_point_ms,
    round_public_range_ms, stream_start_ms,
};
pub use values::{
    CacheCompatibilityId, CommitHash, DescriptorVersion, DoctorCheckName, Identifier, InputRef,
    InputRole, InvalidValue, LicenseExpression, MediaType, OutputContractVersion, PolicyRef,
    ProtocolVersion, RelativeRef, Sha256Digest, StageName, ToolName, UtcTimestamp,
};

/// Frozen Host-facing protocol version.
pub const PROTOCOL_VERSION: &str = "0.1";
/// Frozen Artifact Manifest version.
pub const MANIFEST_VERSION: &str = "0.1";
/// Hard per-line limit for control JSONL.
pub const MAX_CONTROL_LINE_BYTES: usize = 1024 * 1024;
