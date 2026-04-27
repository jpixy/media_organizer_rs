//! FFprobe preflight check.

use super::{CheckResult, CheckSeverity};
use crate::services::ffprobe;

/// Check if ffprobe is installed.
pub fn check() -> CheckResult {
    if ffprobe::is_installed() {
        match ffprobe::get_version() {
            Ok(version) => CheckResult::ok("ffprobe", &format!("installed ({})", version), CheckSeverity::Required),
            Err(_) => CheckResult::ok("ffprobe", "installed", CheckSeverity::Required),
        }
    } else {
        CheckResult::fail(
            "ffprobe",
            "not found",
            "Install FFmpeg: sudo apt install ffmpeg",
            CheckSeverity::Required
        )
    }
}
