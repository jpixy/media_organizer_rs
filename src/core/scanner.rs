//! Directory scanner module.
//!
//! Scans directories recursively for video files, identifying samples
//! and empty directories.

use crate::models::media::VideoFile;
use crate::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Supported video file extensions.
const VIDEO_EXTENSIONS: &[&str] = &[
    // Common formats
    "mkv", "mp4", "avi", "mov", "wmv", // Additional formats
    "m4v", "ts", "m2ts", "flv", "webm", // Less common but supported
    "mpg", "mpeg", "vob", "ogv", "ogm", "divx", "xvid", "3gp", "3g2", "mts", "rm", "rmvb", "asf",
    "f4v",
];

/// Result of scanning a directory.
#[derive(Debug, Default)]
pub struct ScanResult {
    /// Video files found (excluding samples).
    pub videos: Vec<VideoFile>,
    /// Sample files found.
    pub samples: Vec<VideoFile>,
    /// Empty directories found.
    pub empty_dirs: Vec<PathBuf>,
    /// Total files scanned.
    pub total_files_scanned: usize,
    /// Total directories scanned.
    pub total_dirs_scanned: usize,
    /// Organized TV series folders (containing tvshow.nfo)
    pub organized_tv_folders: Vec<PathBuf>,
}

impl ScanResult {
    /// Get total video count (including samples).
    pub fn total_videos(&self) -> usize {
        self.videos.len() + self.samples.len()
    }
}

/// Check if a file extension is a video format.
fn is_video_extension(ext: &str) -> bool {
    let ext_lower = ext.to_lowercase();
    VIDEO_EXTENSIONS.contains(&ext_lower.as_str())
}

/// Check if a file is inside an "Extras" directory.
///
/// Extras directories contain behind-the-scenes content, deleted scenes, etc.
/// These should not be processed as main movies.
///
/// Patterns matched (case-insensitive):
/// - "Extras", "Extra"
/// - "Featurettes", "Featurette"
/// - "Behind the Scenes", "BehindTheScenes"
/// - "Deleted Scenes", "DeletedScenes"
/// - "Making of", "MakingOf"
/// - "Bonus", "Bonuses"
/// - "Special Features"
/// - "Sample", "Samples" (video samples/previews)
/// - Directory names ending with "-Extras" or ".Extras" (e.g., "The.Bourne.Identity.Extras-Grym")
fn is_in_extras_directory(path: &Path) -> bool {
    // Check each component of the path
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy().to_lowercase();

            // Exact matches (case-insensitive)
            let extras_names = [
                "extras",
                "extra",
                "featurettes",
                "featurette",
                "behind the scenes",
                "behindthescenes",
                "deleted scenes",
                "deletedscenes",
                "making of",
                "makingof",
                "bonus",
                "bonuses",
                "special features",
                "specialfeatures",
                "sample",
                "samples",
            ];

            if extras_names.iter().any(|&n| name_str == n) {
                return true;
            }

            // Pattern: ends with ".extras" or "-extras" or "_extras"
            // e.g., "The.Bourne.Identity.Extras-Grym"
            if name_str.contains(".extras")
                || name_str.contains("-extras")
                || name_str.contains("_extras")
            {
                return true;
            }

            // Pattern: ends with ".featurettes" or similar
            if name_str.contains(".featurette") || name_str.contains("-featurette") {
                return true;
            }

            // Pattern: ends with ".sample" or "-sample"
            if name_str.contains(".sample") || name_str.contains("-sample") {
                return true;
            }
        }
    }

    false
}

/// Check if a path component indicates a sample.
/// Matches "sample", "samples", or paths containing "sample" folder.
fn is_sample_path(path: &Path) -> bool {
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let name_lower = name.to_string_lossy().to_lowercase();
            if name_lower == "sample" || name_lower == "samples" {
                return true;
            }
        }
    }
    false
}

/// Check if a filename indicates a sample file.
/// Matches filenames containing "sample" (case-insensitive).
fn is_sample_filename(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    // Check for common sample patterns
    lower.contains("sample") && !lower.contains("sampler")
}

/// Create a VideoFile from a path.
fn create_video_file(path: &Path) -> Result<VideoFile> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata
        .modified()
        .map(chrono::DateTime::<chrono::Utc>::from)
        .unwrap_or_else(|_| chrono::Utc::now());

    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let parent_dir = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let is_sample = is_sample_path(path) || is_sample_filename(&filename);

    Ok(VideoFile {
        path: path.to_path_buf(),
        filename,
        size: metadata.len(),
        modified,
        is_sample,
        parent_dir,
    })
}

/// Scan a directory for video files.
///
/// This function recursively scans the given directory and returns:
/// - All video files (excluding samples)
/// - Sample files (in Sample folders or with "sample" in filename)
/// - Empty directories
///
/// # Arguments
/// * `path` - The directory path to scan
///
/// # Returns
/// A `ScanResult` containing categorized files and directories.
pub fn scan_directory(path: &Path) -> Result<ScanResult> {
    // Validate path exists and is a directory
    if !path.exists() {
        return Err(crate::Error::PathNotFound(path.display().to_string()));
    }
    if !path.is_dir() {
        return Err(crate::Error::NotADirectory(path.display().to_string()));
    }

    let mut result = ScanResult::default();
    let mut dirs_with_files: HashSet<PathBuf> = HashSet::new();
    let mut all_dirs: HashSet<PathBuf> = HashSet::new();

    // Walk the directory tree
    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let entry_path = entry.path();

        if entry.file_type().is_dir() {
            result.total_dirs_scanned += 1;
            all_dirs.insert(entry_path.to_path_buf());
            
            // Check if this directory is an organized TV series folder (contains tvshow.nfo)
            if entry_path.join("tvshow.nfo").exists() {
                result.organized_tv_folders.push(entry_path.to_path_buf());
            }
        } else if entry.file_type().is_file() {
            result.total_files_scanned += 1;

            // Skip files in "Extras" directories - they will be moved as-is with the movie
            // (handled separately in add_extras_operations, similar to subtitles)
            if is_in_extras_directory(entry_path) {
                tracing::debug!(
                    "Extras file (will be moved with movie): {}",
                    entry_path.display()
                );
                continue;
            }

            // Check if it's a video file
            if let Some(ext) = entry_path.extension() {
                if is_video_extension(&ext.to_string_lossy()) {
                    // Get filename for sample check
                    let filename = entry_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");

                    // Sample files with "sample" in the filename are collected separately
                    // and will be moved with the movie via add_subtitle_operations
                    if is_sample_filename(filename) {
                        match create_video_file(entry_path) {
                            Ok(video_file) => {
                                tracing::debug!(
                                    "Sample file (will be moved with movie): {}",
                                    entry_path.display()
                                );
                                result.samples.push(video_file);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to read sample file {:?}: {}", entry_path, e);
                            }
                        }
                        continue;
                    }

                    match create_video_file(entry_path) {
                        Ok(video_file) => {
                            // Mark parent directory as having files
                            if let Some(parent) = entry_path.parent() {
                                dirs_with_files.insert(parent.to_path_buf());
                            }

                            // Regular video
                            result.videos.push(video_file);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to read video file {:?}: {}", entry_path, e);
                        }
                    }
                }
            }
        }
    }

    // Find empty directories (directories with no video files)
    for dir in all_dirs {
        // Skip the root scan directory itself
        if dir == path {
            continue;
        }

        // Check if this directory or any subdirectory has video files
        let has_videos = dirs_with_files.iter().any(|d| d.starts_with(&dir));
        if !has_videos {
            // Double-check it's truly empty (no files at all)
            let is_empty = std::fs::read_dir(&dir)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false);
            if is_empty {
                result.empty_dirs.push(dir);
            }
        }
    }

    // Sort results for consistent output
    result.videos.sort_by(|a, b| a.path.cmp(&b.path));
    result.samples.sort_by(|a, b| a.path.cmp(&b.path));
    result.empty_dirs.sort();

    tracing::info!(
        "Scanned {} files in {} directories: {} videos, {} samples, {} empty dirs",
        result.total_files_scanned,
        result.total_dirs_scanned,
        result.videos.len(),
        result.samples.len(),
        result.empty_dirs.len()
    );

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_video_extension() {
        assert!(is_video_extension("mkv"));
        assert!(is_video_extension("MKV"));
        assert!(is_video_extension("mp4"));
        assert!(is_video_extension("avi"));
        assert!(!is_video_extension("txt"));
        assert!(!is_video_extension("jpg"));
        assert!(!is_video_extension("srt"));
    }

    #[test]
    fn test_is_sample_path() {
        assert!(is_sample_path(Path::new("/movies/Movie/Sample/video.mkv")));
        assert!(is_sample_path(Path::new("/movies/Movie/sample/video.mkv")));
        assert!(is_sample_path(Path::new("/movies/Movie/Samples/video.mkv")));
        assert!(!is_sample_path(Path::new("/movies/Movie/video.mkv")));
    }

    #[test]
    fn test_is_sample_filename() {
        assert!(is_sample_filename("sample.mkv"));
        assert!(is_sample_filename("Sample-movie.mkv"));
        assert!(is_sample_filename("movie-sample.mkv"));
        assert!(!is_sample_filename("movie.mkv"));
        assert!(!is_sample_filename("sampler.mkv")); // Don't match "sampler"
    }

    // Integration tests for scan_directory() moved to tests/scanner_tests.rs
}
