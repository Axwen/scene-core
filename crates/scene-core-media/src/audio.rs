//! Full-track PCM extraction and WAV header inspection.
//!
//! v1 extracts the whole selected audio stream as WAV/PCM `s16le` at the
//! source sample rate and channel count. The engine derives the material-time
//! mapping from the probed stream start plus the actual sample count reported
//! here.

use crate::process::{ProcessError, ProcessSpec, run};
use crate::toolchain::Toolchain;
use std::fmt;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Internal cap for one extraction; the request deadline still applies.
pub const AUDIO_TIMEOUT: Duration = Duration::from_secs(600);
/// Estimated-output ceiling. Above this the engine rejects before starting.
pub const MAX_OUTPUT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// `pcm_s16le` frames are two bytes per sample.
pub const BYTES_PER_SAMPLE: u64 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    ToolUnavailable,
    ToolFailed,
    Timeout,
    Cancelled,
    ResourceLimit,
    EngineInternal(&'static str),
}

impl fmt::Display for AudioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            AudioError::ToolUnavailable => "ffmpeg is unavailable",
            AudioError::ToolFailed => "audio extraction failed",
            AudioError::Timeout => "audio extraction reached its deadline",
            AudioError::Cancelled => "audio extraction was cancelled",
            AudioError::ResourceLimit => "a resource limit was reached",
            AudioError::EngineInternal(reason) => reason,
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for AudioError {}

/// Header facts of a PCM WAV file. `sample_count` is the frame count of the
/// `data` chunk, not an estimate from the requested duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_count: u64,
}

impl WavInfo {
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        self.sample_count * 1000 / u64::from(self.sample_rate)
    }
}

/// Estimated PCM output size for a known material duration.
pub fn estimate_output_bytes(duration_ms: u64, sample_rate: u32, channels: u16) -> u64 {
    duration_ms
        .saturating_mul(u64::from(sample_rate))
        .saturating_mul(u64::from(channels))
        .saturating_mul(BYTES_PER_SAMPLE)
        / 1000
}

/// Extracts the whole audio stream at `audio_stream_index` into `output`.
pub fn extract(
    toolchain: &Toolchain,
    input: &Path,
    audio_stream_index: u32,
    output: &Path,
    cancellation: Option<Arc<AtomicBool>>,
) -> Result<WavInfo, AudioError> {
    let args: Vec<String> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-i",
        input.to_string_lossy().as_ref(),
        "-map",
        format!("0:{audio_stream_index}").as_str(),
        "-vn",
        "-sn",
        "-dn",
        "-c:a",
        "pcm_s16le",
        "-f",
        "wav",
        "-y",
        output.to_string_lossy().as_ref(),
    ]
    .iter()
    .map(|flag| (*flag).to_owned())
    .collect();
    let mut spec = ProcessSpec::new(toolchain.ffmpeg(), args).with_timeout(AUDIO_TIMEOUT);
    if let Some(flag) = cancellation {
        spec = spec.with_cancellation(flag);
    }
    let result = run(&spec).map_err(|error| match error {
        ProcessError::Spawn(_) => AudioError::ToolUnavailable,
        ProcessError::Wait(_) => AudioError::EngineInternal("ffmpeg wait failed"),
        ProcessError::Read(_) => AudioError::EngineInternal("ffmpeg output could not be read"),
    })?;
    if result.cancelled {
        return Err(AudioError::Cancelled);
    }
    if result.timed_out {
        return Err(AudioError::Timeout);
    }
    if !result.success {
        let stderr = String::from_utf8_lossy(&result.stderr).to_lowercase();
        if stderr.contains("no space left") || stderr.contains("permission denied") {
            return Err(AudioError::ResourceLimit);
        }
        return Err(AudioError::ToolFailed);
    }
    if !output.is_file() {
        return Err(AudioError::ToolFailed);
    }
    wav_info(output).map_err(|_| AudioError::EngineInternal("the extracted WAV header is invalid"))
}

/// Reads only the RIFF header chunks, so multi-gigabyte files stay cheap.
pub fn wav_info(path: &Path) -> std::io::Result<WavInfo> {
    let mut file = std::fs::File::open(path)?;
    let mut header = [0_u8; 12];
    file.read_exact(&mut header)?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err(invalid("not a RIFF/WAVE file"));
    }

    let mut format_tag = None;
    let mut channels = None;
    let mut sample_rate = None;
    loop {
        let mut chunk = [0_u8; 8];
        if file.read_exact(&mut chunk).is_err() {
            break;
        }
        let id = &chunk[0..4];
        let size = u64::from(u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]));
        if id == b"fmt " {
            if size < 16 {
                return Err(invalid("the fmt chunk is too small"));
            }
            let mut fmt = vec![0_u8; 16];
            file.read_exact(&mut fmt)?;
            format_tag = Some(u16::from_le_bytes([fmt[0], fmt[1]]));
            channels = Some(u16::from_le_bytes([fmt[2], fmt[3]]));
            sample_rate = Some(u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]));
            skip(&mut file, size - 16)?;
        } else if id == b"data" {
            let channels = channels.ok_or_else(|| invalid("the data chunk precedes fmt"))?;
            let sample_rate = sample_rate.ok_or_else(|| invalid("the data chunk precedes fmt"))?;
            let format_tag = format_tag.ok_or_else(|| invalid("the data chunk precedes fmt"))?;
            if format_tag != 1 && format_tag != 0xFFFE {
                return Err(invalid("the WAV payload is not PCM"));
            }
            if channels == 0 || sample_rate == 0 {
                return Err(invalid("the WAV declares zero channels or sample rate"));
            }
            let block_align = u64::from(channels) * BYTES_PER_SAMPLE;
            return Ok(WavInfo {
                sample_rate,
                channels,
                sample_count: size / block_align,
            });
        } else {
            skip(&mut file, size + (size % 2))?;
        }
    }
    Err(invalid("the WAV has no data chunk"))
}

fn skip(file: &mut std::fs::File, bytes: u64) -> std::io::Result<()> {
    let bytes = i64::try_from(bytes).map_err(|_| invalid("the chunk is too large"))?;
    file.seek(SeekFrom::Current(bytes))?;
    Ok(())
}

fn invalid(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_wav(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("scene-core-wav-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut file = std::fs::File::create(&path).expect("create");
        file.write_all(bytes).expect("write");
        path
    }

    fn pcm_wav(sample_rate: u32, channels: u16, frames: u32) -> Vec<u8> {
        let data_bytes = frames * u32::from(channels) * 2;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_bytes.to_le_bytes());
        bytes.resize(bytes.len() + data_bytes as usize, 0);
        bytes
    }

    #[test]
    fn reads_pcm_header_facts() {
        let path = write_wav("ok", &pcm_wav(48_000, 2, 48_000));
        let info = wav_info(&path).expect("valid wav");
        assert_eq!(
            info,
            WavInfo {
                sample_rate: 48_000,
                channels: 2,
                sample_count: 48_000
            }
        );
        assert_eq!(info.duration_ms(), 1000);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_truncated_and_non_wav_input() {
        let path = write_wav("truncated", b"RIFF\x00\x00");
        assert!(wav_info(&path).is_err());

        let path = write_wav("not-wav", b"OggS\0\0\0\0\0\0\0\0");
        assert!(wav_info(&path).is_err());

        let mut no_data = pcm_wav(8_000, 1, 0);
        no_data.truncate(36);
        no_data[4..8].copy_from_slice(&36_u32.to_le_bytes());
        let path = write_wav("no-data", &no_data);
        assert!(wav_info(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn estimates_pcm_output_bytes() {
        assert_eq!(estimate_output_bytes(1_000, 48_000, 2), 192_000);
        assert_eq!(estimate_output_bytes(0, 48_000, 2), 0);
        // One hour of 48 kHz stereo is ~691 MB; five hours passes the cap.
        assert_eq!(estimate_output_bytes(3_600_000, 48_000, 2), 691_200_000);
        assert!(estimate_output_bytes(18_000_000, 48_000, 2) > MAX_OUTPUT_BYTES);
    }
}
