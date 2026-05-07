//! FFprobe service for extracting video metadata.

use crate::models::media::VideoMetadata;
use crate::Result;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// FFprobe output format.
#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
    format: FfprobeFormat,
}

/// FFprobe stream information.
#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: String,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(default)]
    bits_per_raw_sample: Option<String>,
    channels: Option<u32>,
    #[allow(dead_code)]
    channel_layout: Option<String>,
}

/// FFprobe format information.
#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    format_name: String,
    #[allow(dead_code)]
    duration: Option<String>,
}

/// Check if ffprobe is installed.
pub fn is_installed() -> bool {
    Command::new("ffprobe")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get ffprobe version.
pub fn get_version() -> Result<String> {
    let output = Command::new("ffprobe").arg("-version").output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next().unwrap_or("unknown");

    Ok(first_line.to_string())
}

/// Extract video metadata using ffprobe.
pub fn extract_metadata(path: &Path) -> Result<VideoMetadata> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()?;

    if !output.status.success() {
        return Err(crate::Error::other(format!(
            "ffprobe failed for: {:?}",
            path
        )));
    }

    let ffprobe: FfprobeOutput = serde_json::from_slice(&output.stdout)?;

    // Find video stream
    let video_stream = ffprobe.streams.iter().find(|s| s.codec_type == "video");

    // Find audio stream
    let audio_stream = ffprobe.streams.iter().find(|s| s.codec_type == "audio");

    // Extract width and height
    let (width, height) = video_stream
        .and_then(|s| match (s.width, s.height) {
            (Some(w), Some(h)) => Some((w, h)),
            _ => None,
        })
        .unwrap_or((0, 0));

    // Extract resolution category
    let resolution = if width > 0 && height > 0 {
        resolution_to_string(width, height)
    } else {
        "unknown".to_string()
    };

    // Extract video codec
    let video_codec = video_stream
        .and_then(|s| s.codec_name.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // Extract bit depth
    let bit_depth = video_stream
        .and_then(|s| s.bits_per_raw_sample.as_ref())
        .and_then(|b| b.parse().ok())
        .unwrap_or(8);

    // Extract audio codec
    let audio_codec = audio_stream
        .and_then(|s| s.codec_name.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // Extract audio channels
    let audio_channels = audio_stream
        .and_then(|s| s.channels)
        .map(channels_to_string)
        .unwrap_or_else(|| "unknown".to_string());

    // Extract format
    let format = detect_format(&ffprobe.format.format_name, path);

    Ok(VideoMetadata {
        width,
        height,
        resolution,
        format,
        video_codec,
        bit_depth,
        audio_codec,
        audio_channels,
    })
}

/// Convert resolution to standard string (e.g., "2160p", "1080p").
fn resolution_to_string(width: u32, height: u32) -> String {
    if height >= 2160 || width >= 3840 {
        "2160p".to_string()
    } else if height >= 1080 || width >= 1920 {
        "1080p".to_string()
    } else if height >= 720 || width >= 1280 {
        "720p".to_string()
    } else if height >= 480 || width >= 720 {
        "480p".to_string()
    } else {
        format!("{}p", height)
    }
}

/// Convert channel count to string (e.g., "5.1", "7.1").
fn channels_to_string(channels: u32) -> String {
    match channels {
        1 => "1.0".to_string(),
        2 => "2.0".to_string(),
        6 => "5.1".to_string(),
        8 => "7.1".to_string(),
        _ => format!("{}.0", channels),
    }
}

/// Detect video format from format name and file extension.
///
/// Returns "Unknown" when format cannot be reliably determined from the container alone.
/// The actual format (BluRay, WEB-DL, etc.) should be parsed from the filename
/// via `parse_format_from_filename` and merged via `merge_metadata`.
fn detect_format(format_name: &str, _path: &Path) -> String {
    // MKV is a universal container used by BluRay, WEB-DL, HDTV, Remux, etc.
    // We cannot assume the source format from the container alone.
    // Return "Unknown" so that filename-based parsing takes priority.
    if format_name.contains("matroska") {
        "Unknown".to_string()
    } else if format_name.contains("mp4") || format_name.contains("mov") {
        "Unknown".to_string()
    } else if format_name.contains("avi") {
        "Unknown".to_string()
    } else {
        "Unknown".to_string()
    }
}

/// Parse video metadata from filename.
/// This serves as a fallback when ffprobe fails or to supplement ffprobe data.
pub fn parse_metadata_from_filename(filename: &str) -> VideoMetadata {
    let filename_lower = filename.to_lowercase();

    VideoMetadata {
        width: 0,  // Cannot determine from filename
        height: 0, // Cannot determine from filename
        resolution: parse_resolution_from_filename(&filename_lower),
        format: parse_format_from_filename(&filename_lower),
        video_codec: parse_video_codec_from_filename(&filename_lower),
        bit_depth: parse_bit_depth_from_filename(&filename_lower),
        audio_codec: parse_audio_codec_from_filename(&filename_lower),
        audio_channels: parse_audio_channels_from_filename(&filename_lower),
    }
}

/// Parse resolution from filename (e.g., "4k", "2160p", "1080p", "720p").
fn parse_resolution_from_filename(filename: &str) -> String {
    // Common resolution patterns
    let patterns = [
        // 4K variants
        ("4k", "2160p"),
        ("uhd", "2160p"),
        ("2160p", "2160p"),
        ("2160", "2160p"),
        // 1080p variants
        ("1080p", "1080p"),
        ("1080i", "1080p"),
        ("1080", "1080p"),
        ("fullhd", "1080p"),
        ("fhd", "1080p"),
        // 720p variants
        ("720p", "720p"),
        ("720", "720p"),
        ("hd", "720p"), // Be careful, this is last as it's less specific
        // Lower resolutions
        ("480p", "480p"),
        ("576p", "576p"),
        ("dvd", "480p"),
    ];

    for (pattern, resolution) in patterns {
        // Use word boundary matching to avoid false positives
        if filename.contains(pattern) {
            return resolution.to_string();
        }
    }

    "unknown".to_string()
}

/// Parse video format from filename (e.g., "BluRay", "WEB-DL", "HDTV").
fn parse_format_from_filename(filename: &str) -> String {
    let patterns = [
        // BluRay variants
        ("bluray", "BluRay"),
        ("blu-ray", "BluRay"),
        ("bdrip", "BluRay"),
        ("brrip", "BluRay"),
        ("bdremux", "BluRay.Remux"),
        ("remux", "Remux"),
        // WEB variants
        ("web-dl", "WEB-DL"),
        ("webdl", "WEB-DL"),
        ("webrip", "WEBRip"),
        ("web", "WEB"),
        ("amzn", "AMZN.WEB-DL"),
        ("nf", "NF.WEB-DL"),
        ("dsnp", "DSNP.WEB-DL"),
        // TV variants
        ("hdtv", "HDTV"),
        ("pdtv", "PDTV"),
        // DVD variants
        ("dvdrip", "DVDRip"),
        ("dvd", "DVD"),
        // Other
        ("hdrip", "HDRip"),
        ("hdcam", "HDCAM"),
        ("cam", "CAM"),
        // "ts" is intentionally omitted here because it's too short and matches
        // common substrings like "results", "outskirts", etc.
        // TS format is handled separately below with word-boundary matching.
        ("tc", "TC"),
    ];

    for (pattern, format) in patterns {
        if filename.contains(pattern) {
            return format.to_string();
        }
    }

    // Special handling for short patterns that need word-boundary matching
    // to avoid false positives (e.g., "ts" matching "results")
    if regex::Regex::new(r"(?i)(?:^|[\.\s_-])ts(?:[\.\s_-]|$)")
        .map(|re| re.is_match(filename))
        .unwrap_or(false)
    {
        return "TS".to_string();
    }
    if regex::Regex::new(r"(?i)(?:^|[\.\s_-])tc(?:[\.\s_-]|$)")
        .map(|re| re.is_match(filename))
        .unwrap_or(false)
    {
        return "TC".to_string();
    }

    "Unknown".to_string()
}

/// Parse video codec from filename.
fn parse_video_codec_from_filename(filename: &str) -> String {
    let patterns = [
        ("hevc", "hevc"),
        ("h.265", "hevc"),
        ("h265", "hevc"),
        ("x265", "hevc"),
        ("h.264", "h264"),
        ("h264", "h264"),
        ("x264", "h264"),
        ("avc", "h264"),
        ("av1", "av1"),
        ("vp9", "vp9"),
        ("xvid", "xvid"),
        ("divx", "divx"),
    ];

    for (pattern, codec) in patterns {
        if filename.contains(pattern) {
            return codec.to_string();
        }
    }

    "unknown".to_string()
}

/// Parse bit depth from filename.
fn parse_bit_depth_from_filename(filename: &str) -> u8 {
    if filename.contains("10bit") || filename.contains("10-bit") || filename.contains("hi10p") {
        10
    } else if filename.contains("12bit") || filename.contains("12-bit") {
        12
    } else if filename.contains("8bit") || filename.contains("8-bit") {
        8
    } else if filename.contains("hdr")
        || filename.contains("dolby vision")
        || filename.contains("dv")
    {
        10 // HDR content is typically 10-bit
    } else {
        8 // Default to 8-bit
    }
}

/// Parse audio codec from filename.
fn parse_audio_codec_from_filename(filename: &str) -> String {
    let patterns = [
        ("truehd", "TrueHD"),
        ("atmos", "TrueHD.Atmos"),
        ("dts-hd.ma", "DTS-HD.MA"),
        ("dts-hd", "DTS-HD"),
        ("dts-x", "DTS:X"),
        ("dtsx", "DTS:X"),
        ("dts", "DTS"),
        ("dd+", "EAC3"),
        ("ddp", "EAC3"),
        ("eac3", "EAC3"),
        ("dd5.1", "AC3"),
        ("ac3", "AC3"),
        ("aac", "AAC"),
        ("flac", "FLAC"),
        ("lpcm", "LPCM"),
        ("opus", "Opus"),
        ("mp3", "MP3"),
    ];

    for (pattern, codec) in patterns {
        if filename.contains(pattern) {
            return codec.to_string();
        }
    }

    "unknown".to_string()
}

/// Parse audio channels from filename.
fn parse_audio_channels_from_filename(filename: &str) -> String {
    let patterns = [
        ("7.1", "7.1"),
        ("5.1", "5.1"),
        ("2.1", "2.1"),
        ("2.0", "2.0"),
        ("stereo", "2.0"),
        ("mono", "1.0"),
    ];

    for (pattern, channels) in patterns {
        if filename.contains(pattern) {
            return channels.to_string();
        }
    }

    "unknown".to_string()
}

/// Merge two VideoMetadata, preferring values from primary, falling back to secondary.
pub fn merge_metadata(primary: VideoMetadata, secondary: VideoMetadata) -> VideoMetadata {
    VideoMetadata {
        width: if primary.width > 0 {
            primary.width
        } else {
            secondary.width
        },
        height: if primary.height > 0 {
            primary.height
        } else {
            secondary.height
        },
        resolution: if primary.resolution != "unknown" {
            primary.resolution
        } else {
            secondary.resolution
        },
        format: if primary.format != "Unknown" {
            primary.format
        } else {
            secondary.format
        },
        video_codec: if primary.video_codec != "unknown" {
            primary.video_codec
        } else {
            secondary.video_codec
        },
        // Use primary if it has a meaningful value (non-default 8-bit),
        // otherwise fall back to secondary. Default 8-bit is considered "unknown"
        // since ffprobe returns 8 when the field is missing.
        bit_depth: if primary.bit_depth != 8 {
            primary.bit_depth
        } else {
            secondary.bit_depth
        },
        audio_codec: if primary.audio_codec != "unknown" {
            primary.audio_codec
        } else {
            secondary.audio_codec
        },
        audio_channels: if primary.audio_channels != "unknown" {
            primary.audio_channels
        } else {
            secondary.audio_channels
        },
    }
}

#[cfg(test)]
mod filename_parser_tests {
    use super::*;

    #[test]
    fn test_parse_resolution() {
        // Note: These functions expect lowercase input (called via parse_metadata_from_filename)
        assert_eq!(
            parse_resolution_from_filename("movie.2024.4k.bluray.mkv"),
            "2160p"
        );
        assert_eq!(
            parse_resolution_from_filename("movie.2024.2160p.web-dl.mkv"),
            "2160p"
        );
        assert_eq!(
            parse_resolution_from_filename("movie.2024.1080p.bluray.mkv"),
            "1080p"
        );
        assert_eq!(
            parse_resolution_from_filename("movie.2024.720p.hdtv.mkv"),
            "720p"
        );
        assert_eq!(parse_resolution_from_filename("电影.4k.mp4"), "2160p");
    }

    #[test]
    fn test_parse_format() {
        // Note: These functions expect lowercase input
        assert_eq!(
            parse_format_from_filename("movie.2024.bluray.mkv"),
            "BluRay"
        );
        assert_eq!(
            parse_format_from_filename("movie.2024.web-dl.mkv"),
            "WEB-DL"
        );
        assert_eq!(parse_format_from_filename("movie.2024.hdtv.mkv"), "HDTV");
        // "web-dl" is matched first (order priority), so AMZN prefix is not returned
        assert_eq!(
            parse_format_from_filename("movie.amzn.web-dl.mkv"),
            "WEB-DL"
        );
        // AMZN is matched when there's no explicit web-dl
        assert_eq!(
            parse_format_from_filename("movie.amzn.1080p.mkv"),
            "AMZN.WEB-DL"
        );
    }

    #[test]
    fn test_parse_codec() {
        // Note: These functions expect lowercase input
        assert_eq!(parse_video_codec_from_filename("movie.x265.mkv"), "hevc");
        assert_eq!(parse_video_codec_from_filename("movie.h.264.mkv"), "h264");
        assert_eq!(parse_video_codec_from_filename("movie.hevc.mkv"), "hevc");
    }

    #[test]
    fn test_parse_audio() {
        // Note: These functions expect lowercase input
        assert_eq!(
            parse_audio_codec_from_filename("movie.dts-hd.ma.mkv"),
            "DTS-HD.MA"
        );
        // "truehd" is matched first (order priority), "atmos" suffix is separate pattern
        assert_eq!(
            parse_audio_codec_from_filename("movie.truehd.atmos.mkv"),
            "TrueHD"
        );
        assert_eq!(parse_audio_codec_from_filename("movie.dd5.1.mkv"), "AC3");
    }
}