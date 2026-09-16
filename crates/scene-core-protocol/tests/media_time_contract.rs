//! Pure time-conversion vectors from the confirmed material time policy.
//!
//! Real FFmpeg extraction, player alignment and audio mapping stay in
//! Phase 1 / consumer acceptance; this file only covers the Phase 0 pure
//! conversion contract.

use scene_core_protocol::{
    ContainerInfo, ExactTime, MediaStream, NormalizedMedia, Rational, StreamKind, TimeError,
    container_start_ms, round_public_point_ms, round_public_range_ms, stream_start_ms,
};

fn milliseconds(value: i64) -> ExactTime {
    ExactTime::from_milliseconds(value)
}

fn exact(num: i128, den: i128) -> ExactTime {
    ExactTime::from_parts(num, den).expect("valid exact time")
}

#[test]
fn point_time_rounds_half_away_from_zero() {
    assert_eq!(exact(3, 2000).to_ms_round_nearest(), Ok(2));
    assert_eq!(exact(-3, 2000).to_ms_round_nearest(), Ok(-2));
    assert_eq!(exact(1, 2000).to_ms_round_nearest(), Ok(1));
    assert_eq!(exact(-1, 2000).to_ms_round_nearest(), Ok(-1));
    assert_eq!(round_public_point_ms(exact(3, 2000)), Ok(2));
}

#[test]
fn covering_interval_floors_start_and_ceils_end() {
    let start = exact(6, 5000);
    let end = exact(9, 5000);
    assert_eq!(round_public_range_ms(start, end), Ok((1, 2)));

    let start = exact(10_004, 10_000);
    let end = exact(10_009, 10_000);
    assert_eq!(round_public_range_ms(start, end), Ok((1000, 1001)));
}

#[test]
fn exact_bounds_are_checked_before_rounding() {
    assert_eq!(
        round_public_point_ms(exact(-2, 5000)),
        Err(TimeError::NegativePublicTime)
    );
    assert_eq!(
        round_public_range_ms(exact(-2, 5000), exact(1, 10_000)),
        Err(TimeError::NegativePublicTime)
    );
    assert_eq!(
        round_public_range_ms(exact(1, 1000), exact(1, 1000)),
        Err(TimeError::InvalidPublicRange)
    );
    assert_eq!(
        round_public_range_ms(exact(3, 1000), exact(1, 1000)),
        Err(TimeError::InvalidPublicRange)
    );
}

#[test]
fn integer_millisecond_values_are_preserved() {
    assert_eq!(round_public_point_ms(milliseconds(60_000)), Ok(60_000));
    assert_eq!(
        round_public_range_ms(milliseconds(0), milliseconds(5000)),
        Ok((0, 5000))
    );
    assert_eq!(
        round_public_range_ms(milliseconds(1234), milliseconds(1235)),
        Ok((1234, 1235))
    );
}

#[test]
fn origin_is_shared_by_audio_and_video_offsets() {
    let origin = milliseconds(10_000);
    assert_eq!(
        stream_start_ms(Some(milliseconds(10_000)), Some(origin)),
        Ok(Some(0))
    );
    assert_eq!(
        stream_start_ms(Some(milliseconds(10_080)), Some(origin)),
        Ok(Some(80))
    );
}

#[test]
fn negative_origin_with_negative_presentation_stays_consistent() {
    let origin = ExactTime::from_seconds(-2);
    assert_eq!(
        stream_start_ms(Some(milliseconds(-1500)), Some(origin)),
        Ok(Some(500))
    );
    assert_eq!(round_public_point_ms(milliseconds(500)), Ok(500));
}

#[test]
fn negative_track_start_is_signed_but_public_request_is_rejected() {
    let origin = ExactTime::ZERO;
    assert_eq!(
        stream_start_ms(Some(milliseconds(-80)), Some(origin)),
        Ok(Some(-80))
    );
    assert_eq!(
        round_public_point_ms(milliseconds(-80)),
        Err(TimeError::NegativePublicTime)
    );
}

#[test]
fn unknown_origin_never_defaults_to_zero() {
    assert_eq!(stream_start_ms(Some(milliseconds(80)), None), Ok(None));
    assert_eq!(stream_start_ms(None, Some(ExactTime::ZERO)), Ok(None));
    assert_eq!(stream_start_ms(None, None), Ok(None));
    assert_eq!(container_start_ms(None), None);
    assert_eq!(container_start_ms(Some(ExactTime::ZERO)), Some(0));
}

#[test]
fn container_start_time_accepts_only_zero_or_null() {
    let mut media = normalized_media(Some(0), None);
    assert!(media.validate().is_ok());

    media.container.start_time_ms = None;
    assert!(media.validate().is_ok());

    media.container.start_time_ms = Some(1);
    assert!(media.validate().is_err());
}

#[test]
fn stream_start_keeps_negative_and_unknown_values() {
    let media = normalized_media(Some(0), Some(-80));
    assert!(media.validate().is_ok());

    let media = normalized_media(Some(0), None);
    assert!(media.validate().is_ok());

    let media = normalized_media(None, Some(-80));
    assert!(media.validate().is_ok());
}

#[test]
fn extreme_ticks_are_controlled() {
    let overflowing = ExactTime::from_ticks(
        i64::MAX,
        Rational {
            num: i64::MAX,
            den: 1,
        },
    )
    .expect("fits i128");
    assert_eq!(overflowing.to_ms_round_nearest(), Err(TimeError::Overflow));
    assert_eq!(round_public_point_ms(overflowing), Err(TimeError::Overflow));

    let far = ExactTime::from_ticks(i64::MAX, Rational { num: 1, den: 1 }).expect("fits");
    assert_eq!(far.to_ms_round_nearest(), Err(TimeError::OutOfRange));
    assert_eq!(
        stream_start_ms(Some(far), Some(ExactTime::ZERO)),
        Err(TimeError::OutOfRange)
    );
}

#[test]
fn invalid_time_base_is_rejected() {
    assert!(Rational { num: 1, den: 0 }.validate().is_err());
    assert!(Rational { num: 1, den: -25 }.validate().is_err());
    assert_eq!(
        ExactTime::from_ticks(1, Rational { num: 1, den: 0 }),
        Err(TimeError::DenominatorNotPositive)
    );
}

fn normalized_media(
    container_start_ms: Option<u64>,
    stream_start_ms: Option<i64>,
) -> NormalizedMedia {
    let stream = MediaStream {
        index: 0,
        kind: StreamKind::Video,
        codec_name: "h264".to_owned(),
        duration_ms: Some(60_000),
        start_time_ms: stream_start_ms,
        time_base: Some(Rational { num: 1, den: 1000 }),
        default_disposition: true,
        attached_picture: false,
        coded_width: Some(1920),
        coded_height: Some(1080),
        display_width: Some(1920),
        display_height: Some(1080),
        rotation_degrees: Some(0),
        average_frame_rate: Some(Rational { num: 30, den: 1 }),
        sample_rate: None,
        channels: None,
        channel_layout: None,
    };
    NormalizedMedia {
        container: ContainerInfo {
            format_name: "mov,mp4,m4a,3gp,3g2,mj2".to_owned(),
            duration_ms: Some(60_000),
            start_time_ms: container_start_ms,
            bit_rate_bps: None,
            file_size_bytes: 1_048_576,
        },
        streams: vec![stream],
        primary_video_stream_index: Some(0),
    }
}
