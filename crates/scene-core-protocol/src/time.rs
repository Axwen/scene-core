//! Exact media-time arithmetic for the confirmed material time policy.
//!
//! Display time is `P = presentation timestamp x time_base`, material time is
//! `T = P - O` with `O` the container presentation origin. Rationals,
//! differences and rounding all use checked integer arithmetic; unknown
//! values stay `None` and are never defaulted to zero.
//!
//! Rounding rules:
//! - point time, metadata starts and durations round to nearest millisecond,
//!   half away from zero (`1.5 -> 2`, `-1.5 -> -2`);
//! - covering intervals floor the start and ceil the end;
//! - public requests and annotations require an exact non-negative start and
//!   `end > start` before any rounding is applied.

use crate::error::ValidationError;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

const MS_PER_SECOND: i128 = 1000;

/// Wire rational number, e.g. a time base. The denominator must be positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Rational {
    pub num: i64,
    pub den: i64,
}

impl Rational {
    /// Validates the denominator without normalizing the caller's value.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.den <= 0 {
            return Err(ValidationError::new("den", "must be a positive integer"));
        }
        Ok(())
    }
}

/// Exact time arithmetic failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeError {
    /// A rational or time base with a non-positive denominator.
    DenominatorNotPositive,
    /// Checked integer arithmetic overflowed.
    Overflow,
    /// The result does not fit the millisecond wire range.
    OutOfRange,
    /// A public request or annotation was negative before rounding.
    NegativePublicTime,
    /// A public interval had `end <= start` before rounding.
    InvalidPublicRange,
}

impl fmt::Display for TimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            TimeError::DenominatorNotPositive => "denominator must be positive",
            TimeError::Overflow => "exact time arithmetic overflowed",
            TimeError::OutOfRange => "millisecond value is out of range",
            TimeError::NegativePublicTime => "public times must be non-negative",
            TimeError::InvalidPublicRange => "public ranges require end > start",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for TimeError {}

/// Exact time in seconds kept as a normalized rational number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactTime {
    num: i128,
    den: i128,
}

impl ExactTime {
    pub const ZERO: Self = Self { num: 0, den: 1 };

    /// Builds an exact time from a raw numerator and denominator.
    pub fn from_parts(num: i128, den: i128) -> Result<Self, TimeError> {
        if den == 0 {
            return Err(TimeError::DenominatorNotPositive);
        }
        let (num, den) = if den < 0 {
            (
                num.checked_neg().ok_or(TimeError::Overflow)?,
                den.checked_neg().ok_or(TimeError::Overflow)?,
            )
        } else {
            (num, den)
        };
        let divisor = gcd(num.unsigned_abs(), den.unsigned_abs());
        let (num, den) = match i128::try_from(divisor) {
            Ok(divisor) if divisor > 1 => (num / divisor, den / divisor),
            _ => (num, den),
        };
        Ok(Self { num, den })
    }

    /// Converts an integer millisecond value into exact time.
    pub fn from_milliseconds(milliseconds: i64) -> Self {
        Self::from_parts(i128::from(milliseconds), MS_PER_SECOND)
            .expect("millisecond denominator is positive")
    }

    /// Converts an integer second value into exact time.
    pub fn from_seconds(seconds: i64) -> Self {
        Self {
            num: i128::from(seconds),
            den: 1,
        }
    }

    /// `timestamp x time_base` with checked arithmetic.
    pub fn from_ticks(timestamp: i64, time_base: Rational) -> Result<Self, TimeError> {
        if time_base.den <= 0 {
            return Err(TimeError::DenominatorNotPositive);
        }
        let num = i128::from(timestamp)
            .checked_mul(i128::from(time_base.num))
            .ok_or(TimeError::Overflow)?;
        Self::from_parts(num, i128::from(time_base.den))
    }

    pub fn checked_add(&self, other: &Self) -> Result<Self, TimeError> {
        let left = self.num.checked_mul(other.den).ok_or(TimeError::Overflow)?;
        let right = other.num.checked_mul(self.den).ok_or(TimeError::Overflow)?;
        let denominator = self.den.checked_mul(other.den).ok_or(TimeError::Overflow)?;
        Self::from_parts(
            left.checked_add(right).ok_or(TimeError::Overflow)?,
            denominator,
        )
    }

    pub fn checked_sub(&self, other: &Self) -> Result<Self, TimeError> {
        let negated = other.checked_neg()?;
        self.checked_add(&negated)
    }

    pub fn checked_neg(&self) -> Result<Self, TimeError> {
        Ok(Self {
            num: self.num.checked_neg().ok_or(TimeError::Overflow)?,
            den: self.den,
        })
    }

    pub fn is_negative(&self) -> bool {
        self.num < 0
    }

    /// Exact ordering without rounding.
    pub fn checked_cmp(&self, other: &Self) -> Result<Ordering, TimeError> {
        let left = self.num.checked_mul(other.den).ok_or(TimeError::Overflow)?;
        let right = other.num.checked_mul(self.den).ok_or(TimeError::Overflow)?;
        Ok(left.cmp(&right))
    }

    /// Rounds to the nearest millisecond, half away from zero.
    pub fn to_ms_round_nearest(&self) -> Result<i64, TimeError> {
        let scaled = self
            .num
            .checked_mul(MS_PER_SECOND)
            .ok_or(TimeError::Overflow)?;
        let magnitude = scaled.unsigned_abs();
        let denominator = self.den as u128;
        let mut quotient = magnitude / denominator;
        let remainder = magnitude % denominator;
        if remainder >= denominator - remainder {
            quotient += 1;
        }
        let quotient = i128::try_from(quotient).map_err(|_| TimeError::Overflow)?;
        let signed = if scaled < 0 {
            quotient.checked_neg().ok_or(TimeError::Overflow)?
        } else {
            quotient
        };
        i64::try_from(signed).map_err(|_| TimeError::OutOfRange)
    }

    /// Rounds down to a whole millisecond.
    pub fn to_ms_floor(&self) -> Result<i64, TimeError> {
        let scaled = self
            .num
            .checked_mul(MS_PER_SECOND)
            .ok_or(TimeError::Overflow)?;
        i64::try_from(scaled.div_euclid(self.den)).map_err(|_| TimeError::OutOfRange)
    }

    /// Rounds up to a whole millisecond.
    pub fn to_ms_ceil(&self) -> Result<i64, TimeError> {
        let scaled = self
            .num
            .checked_mul(MS_PER_SECOND)
            .ok_or(TimeError::Overflow)?;
        let quotient = scaled.div_euclid(self.den);
        let remainder = scaled.rem_euclid(self.den);
        let ceiling = if remainder == 0 {
            quotient
        } else {
            quotient.checked_add(1).ok_or(TimeError::Overflow)?
        };
        i64::try_from(ceiling).map_err(|_| TimeError::OutOfRange)
    }
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// Rounds a public point time: exact non-negative check first, then nearest
/// millisecond with half away from zero.
pub fn round_public_point_ms(exact: ExactTime) -> Result<u64, TimeError> {
    if exact.is_negative() {
        return Err(TimeError::NegativePublicTime);
    }
    u64::try_from(exact.to_ms_round_nearest()?).map_err(|_| TimeError::OutOfRange)
}

/// Rounds a public covering interval: exact `start >= 0` and `end > start`
/// checks first, then floor(start) and ceil(end).
pub fn round_public_range_ms(start: ExactTime, end: ExactTime) -> Result<(u64, u64), TimeError> {
    if start.is_negative() {
        return Err(TimeError::NegativePublicTime);
    }
    if start.checked_cmp(&end)? != Ordering::Less {
        return Err(TimeError::InvalidPublicRange);
    }
    let start_ms = u64::try_from(start.to_ms_floor()?).map_err(|_| TimeError::OutOfRange)?;
    let end_ms = u64::try_from(end.to_ms_ceil()?).map_err(|_| TimeError::OutOfRange)?;
    if end_ms <= start_ms {
        return Err(TimeError::InvalidPublicRange);
    }
    Ok((start_ms, end_ms))
}

/// Container presentation start: `0` when the origin is known, otherwise null.
pub fn container_start_ms(origin: Option<ExactTime>) -> Option<u64> {
    origin.map(|_| 0)
}

/// Signed track start relative to the container origin. Unknown presentation
/// or origin values stay `None`; they are never defaulted to zero.
pub fn stream_start_ms(
    presentation: Option<ExactTime>,
    origin: Option<ExactTime>,
) -> Result<Option<i64>, TimeError> {
    let (Some(presentation), Some(origin)) = (presentation, origin) else {
        return Ok(None);
    };
    Ok(Some(
        presentation.checked_sub(&origin)?.to_ms_round_nearest()?,
    ))
}
