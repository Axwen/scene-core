//! Validated wire values shared by every Protocol 0.1 DTO.
//!
//! Each type rejects malformed input while deserializing, so a parsed DTO never
//! carries an out-of-contract identifier, digest, ref, timestamp or media type.

use crate::PROTOCOL_VERSION;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Failure returned when a validated protocol value does not satisfy its format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidValue {
    what: &'static str,
    reason: &'static str,
}

impl InvalidValue {
    pub(crate) const fn new(what: &'static str, reason: &'static str) -> Self {
        Self { what, reason }
    }

    /// Name of the value that failed validation.
    pub const fn what(&self) -> &'static str {
        self.what
    }

    /// Human-readable reason without echoing the rejected value.
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for InvalidValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {}: {}", self.what, self.reason)
    }
}

impl std::error::Error for InvalidValue {}

macro_rules! validated_string {
    ($(#[$meta:meta])* $name:ident, $what:literal, $validate:path) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            /// Validates `value` and wraps it.
            pub fn new(value: impl Into<String>) -> Result<Self, InvalidValue> {
                let value = value.into();
                $validate(&value).map_err(|reason| InvalidValue::new($what, reason))?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = InvalidValue;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                value
                    .parse()
                    .map_err(|error| <D::Error as serde::de::Error>::custom(error))
            }
        }
    };
}

fn bounded(value: &str, max_len: usize) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("must not be empty");
    }
    if value.len() > max_len {
        return Err("is too long");
    }
    Ok(())
}

fn starts_with(
    value: &str,
    predicate: fn(u8) -> bool,
    reason: &'static str,
) -> Result<(), &'static str> {
    if value.bytes().next().is_some_and(predicate) {
        Ok(())
    } else {
        Err(reason)
    }
}

fn uses_only(
    value: &str,
    predicate: fn(u8) -> bool,
    reason: &'static str,
) -> Result<(), &'static str> {
    if value.bytes().all(predicate) {
        Ok(())
    } else {
        Err(reason)
    }
}

fn lower_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

fn validate_lower_hex(value: &str, expected_len: usize) -> Result<(), &'static str> {
    if value.len() != expected_len {
        return Err("must contain the expected number of lowercase hexadecimal digits");
    }
    uses_only(
        value,
        lower_hex_digit,
        "must use lowercase hexadecimal digits",
    )
}

fn identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
}

fn cache_id_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
}

fn role_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
}

fn validate_identifier(value: &str) -> Result<(), &'static str> {
    bounded(value, 128)?;
    starts_with(
        value,
        |byte| byte.is_ascii_alphanumeric(),
        "must start with an ASCII letter or digit",
    )?;
    uses_only(
        value,
        identifier_byte,
        "may only contain ASCII letters, digits, '.', '_', ':' or '-'",
    )
}

fn validate_commit_hash(value: &str) -> Result<(), &'static str> {
    validate_lower_hex(value, 40)
}

fn validate_cache_compatibility_id(value: &str) -> Result<(), &'static str> {
    bounded(value, 128)?;
    starts_with(
        value,
        |byte| byte.is_ascii_alphanumeric(),
        "must start with an ASCII letter or digit",
    )?;
    uses_only(
        value,
        cache_id_byte,
        "may only contain ASCII letters, digits, '.', '_' or '-'",
    )
}

fn validate_sha256_digest(value: &str) -> Result<(), &'static str> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err("must start with 'sha256:'");
    };
    validate_lower_hex(hex, 64)
}

fn validate_protocol_version(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > 16 {
        return Err("must be 1 to 16 bytes");
    }
    if value.starts_with('.') || value.ends_with('.') || value.contains("..") {
        return Err("must be a dotted numeric version");
    }
    uses_only(
        value,
        |byte| byte.is_ascii_digit() || byte == b'.',
        "must be a dotted numeric version",
    )
}

fn validate_output_contract_version(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > 64 {
        return Err("must be 1 to 64 bytes");
    }
    let Some((name, revision)) = value.split_once('/') else {
        return Err("must be formatted as '<name>/<revision>'");
    };
    if name.is_empty() || revision.is_empty() || revision.contains('/') {
        return Err("must be formatted as '<name>/<revision>'");
    }
    uses_only(
        name,
        |byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-',
        "the name may only contain lowercase ASCII letters, digits or '-'",
    )?;
    uses_only(
        revision,
        |byte| byte.is_ascii_digit(),
        "the revision must be numeric",
    )
}

fn validate_relative_ref(value: &str) -> Result<(), &'static str> {
    bounded(value, 1024)?;
    if value.starts_with('/') {
        return Err("must be relative, not absolute");
    }
    if value.contains('\\') {
        return Err("must use forward slashes");
    }
    if value.contains(':') {
        return Err("must not contain a drive letter or alternate data stream separator");
    }
    if value.ends_with('/') {
        return Err("must not end with '/'");
    }
    if value.chars().any(char::is_control) {
        return Err("must not contain control characters");
    }
    for segment in value.split('/') {
        if segment.is_empty() {
            return Err("must not contain empty path segments");
        }
        if segment == "." || segment == ".." {
            return Err("must not contain '.' or '..' path segments");
        }
    }
    Ok(())
}

fn validate_input_ref(value: &str) -> Result<(), &'static str> {
    if value != "input/source.media" {
        return Err("Protocol 0.1 requires the exact ref 'input/source.media'");
    }
    Ok(())
}

fn validate_policy_ref(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > 200 {
        return Err("must be 1 to 200 bytes");
    }
    if value.chars().any(char::is_control) {
        return Err("must not contain control characters");
    }
    let Some((name, version)) = value.rsplit_once('@') else {
        return Err("must be formatted as '<name>@<version>'");
    };
    if name.is_empty() || version.is_empty() {
        return Err("both the name and the version must be non-empty");
    }
    if name.contains('@') {
        return Err("must contain a single '@' separator");
    }
    validate_relative_ref(name).map_err(|_| "name must be a safe relative reference")?;
    uses_only(
        version,
        |byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'),
        "version may only contain ASCII letters, digits, '.', '_' or '-'",
    )
}

fn validate_media_type(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > 128 {
        return Err("must be 1 to 128 bytes");
    }
    let Some((kind, subtype)) = value.split_once('/') else {
        return Err("must contain exactly one '/'");
    };
    if kind.is_empty() || subtype.is_empty() || subtype.contains('/') {
        return Err("must contain a non-empty type and subtype");
    }
    let token = |part: &str| {
        part.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'+' | b'-' | b'_')
        })
    };
    if !token(kind) || !token(subtype) {
        return Err("must use lowercase ASCII token characters");
    }
    Ok(())
}

fn validate_stage_name(value: &str) -> Result<(), &'static str> {
    bounded(value, 64)?;
    starts_with(
        value,
        |byte| byte.is_ascii_alphanumeric(),
        "must start with an ASCII letter or digit",
    )?;
    uses_only(
        value,
        cache_id_byte,
        "may only contain ASCII letters, digits, '.', '_' or '-'",
    )
}

fn validate_tool_name(value: &str) -> Result<(), &'static str> {
    bounded(value, 32)?;
    starts_with(
        value,
        |byte| byte.is_ascii_lowercase(),
        "must start with a lowercase ASCII letter",
    )?;
    uses_only(
        value,
        |byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-',
        "may only contain lowercase ASCII letters, digits or '-'",
    )
}

fn validate_license_expression(value: &str) -> Result<(), &'static str> {
    bounded(value, 128)?;
    uses_only(
        value,
        |byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-' | b'(' | b')' | b' ')
        },
        "must be an SPDX-like license expression",
    )
}

fn validate_check_name(value: &str) -> Result<(), &'static str> {
    bounded(value, 64)?;
    starts_with(
        value,
        |byte| byte.is_ascii_lowercase(),
        "must start with a lowercase ASCII letter",
    )?;
    uses_only(
        value,
        |byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-',
        "may only contain lowercase ASCII letters, digits or '-'",
    )
}

fn validate_input_role(value: &str) -> Result<(), &'static str> {
    bounded(value, 64)?;
    starts_with(
        value,
        |byte| byte.is_ascii_lowercase(),
        "must start with a lowercase ASCII letter",
    )?;
    uses_only(
        value,
        role_byte,
        "may only contain lowercase ASCII letters, digits, '_' or '-'",
    )
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn parse_decimal(bytes: &[u8], start: usize, len: usize) -> Option<u32> {
    let slice = bytes.get(start..start.checked_add(len)?)?;
    let mut value = 0_u32;
    for byte in slice {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + u32::from(byte - b'0');
    }
    Some(value)
}

fn validate_utc_timestamp(value: &str) -> Result<(), &'static str> {
    let bytes = value.as_bytes();
    if bytes.len() < 20 {
        return Err("must be an RFC 3339 timestamp");
    }
    if bytes[4] != b'-' || bytes[7] != b'-' || bytes[13] != b':' || bytes[16] != b':' {
        return Err("must be an RFC 3339 timestamp");
    }
    if !matches!(bytes[10], b'T' | b't') {
        return Err("must separate the date and time with 'T'");
    }
    let year = parse_decimal(bytes, 0, 4).ok_or("year must be four digits")?;
    let month = parse_decimal(bytes, 5, 2).ok_or("month must be two digits")?;
    let day = parse_decimal(bytes, 8, 2).ok_or("day must be two digits")?;
    let hour = parse_decimal(bytes, 11, 2).ok_or("hour must be two digits")?;
    let minute = parse_decimal(bytes, 14, 2).ok_or("minute must be two digits")?;
    let second = parse_decimal(bytes, 17, 2).ok_or("second must be two digits")?;
    if !(1..=12).contains(&month) {
        return Err("month must be between 1 and 12");
    }
    if day == 0 || day > days_in_month(year, month) {
        return Err("day is out of range for the month");
    }
    if hour > 23 {
        return Err("hour must be between 0 and 23");
    }
    if minute > 59 {
        return Err("minute must be between 0 and 59");
    }
    if second > 60 {
        return Err("second must be between 0 and 60");
    }
    let mut index = 19;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return Err("fractional seconds must contain at least one digit");
        }
    }
    match bytes.get(index) {
        Some(b'Z' | b'z') => index += 1,
        Some(b'+' | b'-') => {
            let offset_hour = parse_decimal(bytes, index + 1, 2)
                .ok_or("time zone offset hour must be two digits")?;
            if bytes.get(index + 3) != Some(&b':') {
                return Err("time zone offset must use the form +HH:MM");
            }
            let offset_minute = parse_decimal(bytes, index + 4, 2)
                .ok_or("time zone offset minute must be two digits")?;
            if offset_hour > 23 || offset_minute > 59 {
                return Err("time zone offset is out of range");
            }
            index += 6;
        }
        _ => return Err("must end with a 'Z' or numeric time zone offset"),
    }
    if index != bytes.len() {
        return Err("must not contain trailing characters");
    }
    Ok(())
}

validated_string!(
    /// Protocol identifier: 1-128 characters matching
    /// `[A-Za-z0-9][A-Za-z0-9._:-]{0,127}`.
    ///
    /// Used for `requestId`, `sourceVersionId`, `runId`, `segmentId` and
    /// `artifactId`.
    Identifier,
    "identifier",
    validate_identifier
);

validated_string!(
    /// Lowercase 40-digit hexadecimal engine commit hash.
    CommitHash,
    "commit hash",
    validate_commit_hash
);

validated_string!(
    /// Engine cache compatibility identifier such as `scene-core-output-v1`.
    CacheCompatibilityId,
    "cache compatibility id",
    validate_cache_compatibility_id
);

validated_string!(
    /// `sha256:` followed by exactly 64 lowercase hexadecimal digits.
    Sha256Digest,
    "SHA-256 digest",
    validate_sha256_digest
);

validated_string!(
    /// Dotted numeric protocol version. Protocol 0.1 requires exact `0.1`.
    ProtocolVersion,
    "protocol version",
    validate_protocol_version
);

validated_string!(
    /// Registered operation result contract such as `probe-result/1`.
    OutputContractVersion,
    "output contract version",
    validate_output_contract_version
);

validated_string!(
    /// Safe relative POSIX reference. Absolute paths, backslashes, drive
    /// letters, alternate data streams, control characters and `.`/`..`
    /// segments are rejected.
    RelativeRef,
    "relative reference",
    validate_relative_ref
);

validated_string!(
    /// Protocol 0.1 input reference. Only `input/source.media` is accepted.
    InputRef,
    "input reference",
    validate_input_ref
);

validated_string!(
    /// Versioned segmentation policy reference such as
    /// `shot-boundary/example@1`.
    PolicyRef,
    "policy reference",
    validate_policy_ref
);

validated_string!(
    /// Lowercase media type such as `image/jpeg`.
    MediaType,
    "media type",
    validate_media_type
);

validated_string!(
    /// Stable progress stage name.
    StageName,
    "stage name",
    validate_stage_name
);

validated_string!(
    /// Input role token such as `source_media`.
    InputRole,
    "input role",
    validate_input_role
);

validated_string!(
    /// RFC 3339 UTC timestamp such as `2026-09-07T00:00:00Z`.
    UtcTimestamp,
    "timestamp",
    validate_utc_timestamp
);

validated_string!(
    /// Fixed tool name from the package manifest, such as `ffmpeg`.
    ToolName,
    "tool name",
    validate_tool_name
);

validated_string!(
    /// SPDX-like license expression such as `LGPL-2.1-or-later`.
    LicenseExpression,
    "license expression",
    validate_license_expression
);

validated_string!(
    /// Stable doctor check name such as `package-manifest`.
    DoctorCheckName,
    "doctor check name",
    validate_check_name
);

/// Frozen descriptor schema revision. Protocol 0.1 registers only `"1"` for
/// both `inputSetVersion` and `derivationDescriptorVersion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum DescriptorVersion {
    #[serde(rename = "1")]
    V1,
}

impl DescriptorVersion {
    /// Revision frozen by this crate.
    pub const fn current() -> Self {
        Self::V1
    }

    pub const fn as_str(self) -> &'static str {
        "1"
    }
}

impl fmt::Display for DescriptorVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

macro_rules! string_schema {
    ($name:ident, $schema:tt) => {
        impl schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                std::borrow::Cow::Borrowed(stringify!($name))
            }

            fn schema_id() -> std::borrow::Cow<'static, str> {
                std::borrow::Cow::Borrowed(concat!(
                    "scene_core_protocol::values::",
                    stringify!($name)
                ))
            }

            fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!($schema)
            }
        }
    };
}

string_schema!(
    Identifier,
    {"type": "string", "pattern": "^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$"}
);
string_schema!(CommitHash, {"type": "string", "pattern": "^[0-9a-f]{40}$"});
string_schema!(
    CacheCompatibilityId,
    {"type": "string", "pattern": "^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$"}
);
string_schema!(
    Sha256Digest,
    {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"}
);
string_schema!(ProtocolVersion, {"type": "string", "const": "0.1"});
string_schema!(
    OutputContractVersion,
    {"type": "string", "pattern": "^[a-z0-9-]+/[0-9]+$"}
);
string_schema!(
    RelativeRef,
    {"type": "string", "minLength": 1, "maxLength": 1024}
);
string_schema!(InputRef, {"type": "string", "const": "input/source.media"});
string_schema!(
    PolicyRef,
    {"type": "string", "pattern": "^[^@]+@[A-Za-z0-9._-]+$", "maxLength": 200}
);
string_schema!(
    MediaType,
    {"type": "string", "pattern": "^[a-z0-9.+-_]+/[a-z0-9.+-_]+$"}
);
string_schema!(
    StageName,
    {"type": "string", "pattern": "^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$"}
);
string_schema!(InputRole, {"type": "string", "const": "source_media"});
string_schema!(UtcTimestamp, {"type": "string", "format": "date-time"});
string_schema!(
    ToolName,
    {"type": "string", "pattern": "^[a-z][a-z0-9-]{0,31}$"}
);
string_schema!(
    LicenseExpression,
    {"type": "string", "pattern": "^[A-Za-z0-9.()+-]+( [A-Za-z0-9.()+-]+)*$"}
);
string_schema!(
    DoctorCheckName,
    {"type": "string", "pattern": "^[a-z][a-z0-9-]{0,63}$"}
);

impl Sha256Digest {
    /// Hashes `bytes` and returns the canonical `sha256:<hex>` digest.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        use sha2::{Digest as _, Sha256};

        let digest = Sha256::digest(bytes);
        Self(format!("sha256:{}", hex_lower(&digest)))
    }
}

impl ProtocolVersion {
    /// Version frozen by this crate.
    pub fn current() -> Self {
        Self(PROTOCOL_VERSION.to_owned())
    }

    /// Whether this exact version is supported by the current build.
    pub fn is_supported(&self) -> bool {
        self.as_str() == PROTOCOL_VERSION
    }
}

impl InputRole {
    /// Role required for the single Protocol 0.1 input.
    pub const SOURCE_MEDIA: &'static str = "source_media";

    pub fn is_source_media(&self) -> bool {
        self.as_str() == Self::SOURCE_MEDIA
    }
}

pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    use fmt::Write as _;

    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_accepts_contract_examples() {
        for value in ["req_01", "sourcev_01", "run_01", "shot_000001", "a"] {
            assert!(Identifier::new(value).is_ok(), "{value}");
        }
    }

    #[test]
    fn identifier_rejects_invalid_shapes() {
        for value in ["", "-lead", "has space", "emoji🙂", "a/b", "a\\b"] {
            assert!(Identifier::new(value).is_err(), "{value}");
        }
        assert!(Identifier::new("a".repeat(128)).is_ok());
        assert!(Identifier::new("a".repeat(129)).is_err());
    }

    #[test]
    fn digest_roundtrip_and_rejection() {
        let digest = Sha256Digest::from_bytes(b"{}");
        assert_eq!(
            digest.as_str(),
            "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
        assert!(Sha256Digest::new("sha256:ABCDEF").is_err());
        assert!(Sha256Digest::new("md5:00").is_err());
    }

    #[test]
    fn relative_ref_rejects_unsafe_forms() {
        assert!(RelativeRef::new("output/preview/opening.jpg").is_ok());
        for value in [
            "/etc/passwd",
            "C:/media/source.mp4",
            "C:\\media\\source.mp4",
            "\\\\server\\share\\file",
            "input/../../escape",
            "input/file:stream",
            "input//file",
            "input/",
            "input/\u{7}file",
        ] {
            assert!(RelativeRef::new(value).is_err(), "{value}");
        }
    }

    #[test]
    fn input_ref_is_exact() {
        assert!(InputRef::new("input/source.media").is_ok());
        assert!(InputRef::new("input/source.MEDIA").is_err());
        assert!(InputRef::new("input/other.media").is_err());
    }

    #[test]
    fn policy_ref_requires_version_suffix() {
        assert!(PolicyRef::new("shot-boundary/example@1").is_ok());
        assert!(PolicyRef::new("shot-boundary/example").is_err());
        assert!(PolicyRef::new("shot-boundary/example@1@2").is_err());
        assert!(PolicyRef::new("../example@1").is_err());
    }

    #[test]
    fn output_contract_version_requires_name_and_revision() {
        assert!(OutputContractVersion::new("probe-result/1").is_ok());
        assert!(OutputContractVersion::new("extract-preview-result/1").is_ok());
        for value in [
            "probe-result",
            "probe-result/",
            "/1",
            "Probe-Result/1",
            "probe result/1",
        ] {
            assert!(OutputContractVersion::new(value).is_err(), "{value}");
        }
    }

    #[test]
    fn timestamps_accept_rfc3339_and_reject_rest() {
        for value in [
            "2026-09-07T00:00:00Z",
            "2026-09-07T00:00:00.123Z",
            "2024-02-29T23:59:60+08:00",
        ] {
            assert!(UtcTimestamp::new(value).is_ok(), "{value}");
        }
        for value in [
            "2026-09-07",
            "2023-02-29T00:00:00Z",
            "2026-09-07T24:00:00Z",
            "2026-09-07T00:00:00",
            "2026-09-07T00:00:00.Z",
            "2026-09-07T00:00:00Zjunk",
        ] {
            assert!(UtcTimestamp::new(value).is_err(), "{value}");
        }
    }

    #[test]
    fn protocol_version_support_is_exact() {
        assert!(ProtocolVersion::current().is_supported());
        let future = ProtocolVersion::new("0.2").expect("well-formed version");
        assert!(!future.is_supported());
        assert!(ProtocolVersion::new("zero").is_err());
    }

    #[test]
    fn media_type_rejects_parameters_and_uppercase() {
        assert!(MediaType::new("image/jpeg").is_ok());
        for value in ["IMAGE/JPEG", "image", "image/jpeg;q=0.8", "image/", "/jpeg"] {
            assert!(MediaType::new(value).is_err(), "{value}");
        }
    }

    #[test]
    fn descriptor_version_is_serialized_as_literal_one() {
        let value = serde_json::to_value(DescriptorVersion::current()).expect("serializes");
        assert_eq!(value, serde_json::json!("1"));
        let parsed: DescriptorVersion = serde_json::from_value(value).expect("parses");
        assert_eq!(parsed, DescriptorVersion::V1);
        assert!(serde_json::from_value::<DescriptorVersion>(serde_json::json!("2")).is_err());
    }
}
