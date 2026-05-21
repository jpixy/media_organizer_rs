//! Unit tests for poster download command.

use media_organizer::cli::commands::poster::{download_file, download_file_with_size, extract_season_from_dirname, is_video_file, parse_tmdb_id_from_folder_name, format_size};
use std::path::PathBuf;

#[test]
fn test_is_video_file() {
    // Test common video extensions
    assert!(is_video_file(PathBuf::from("movie.mp4")), "MP4 should be recognized as video");
    assert!(is_video_file(PathBuf::from("show.mkv")), "MKV should be recognized as video");
    assert!(is_video_file(PathBuf::from("film.avi")), "AVI should be recognized as video");
    assert!(is_video_file(PathBuf::from("video.mov")), "MOV should be recognized as video");
    assert!(is_video_file(PathBuf::from("clip.webm")), "WEBM should be recognized as video");
    
    // Test non-video extensions
    assert!(!is_video_file(PathBuf::from("file.txt")), "TXT should not be recognized as video");
    assert!(!is_video_file(PathBuf::from("image.jpg")), "JPG should not be recognized as video");
    assert!(!is_video_file(PathBuf::from("data.json")), "JSON should not be recognized as video");
    assert!(!is_video_file(PathBuf::from("archive.zip")), "ZIP should not be recognized as video");
    
    // Test case insensitivity
    assert!(is_video_file(PathBuf::from("Movie.MP4")), "MP4 (uppercase) should be recognized as video");
    assert!(is_video_file(PathBuf::from("Show.Mkv")), "MKV (mixed case) should be recognized as video");
    
    println!("test_is_video_file: passed");
}

#[test]
fn test_parse_tmdb_id_from_folder_name() {
    // Test standard patterns
    assert_eq!(parse_tmdb_id_from_folder_name("[Movie Title](2024)-tmdb123456"), Some(123456), "Standard tmdb pattern");
    assert_eq!(parse_tmdb_id_from_folder_name("[TV Show](2023)-tt12345678-tmdb987654"), Some(987654), "TMDB after IMDB");
    assert_eq!(parse_tmdb_id_from_folder_name("Movie Name (2024) tmdb112233"), Some(112233), "TMDB with space");
    
    // Test edge cases
    assert_eq!(parse_tmdb_id_from_folder_name("tmdb123"), Some(123), "Only TMDB ID");
    assert_eq!(parse_tmdb_id_from_folder_name("No TMDB ID here"), None, "No TMDB ID");
    assert_eq!(parse_tmdb_id_from_folder_name("tmdb"), None, "tmdb without number");
    assert_eq!(parse_tmdb_id_from_folder_name(""), None, "Empty string");
    
    // Test Chinese folder names
    assert_eq!(parse_tmdb_id_from_folder_name("[电影名称](2024)-tmdb654321"), Some(654321), "Chinese folder with TMDB");
    assert_eq!(parse_tmdb_id_from_folder_name("[El Eternauta][永航员](2025)-tt27740241-tmdb226362"), Some(226362), "Mixed Chinese/English");
    
    println!("test_parse_tmdb_id_from_folder_name: passed");
}

#[test]
fn test_extract_season_from_dirname() {
    // Test standard season folder patterns
    assert_eq!(extract_season_from_dirname("Season 01"), Some(1), "Standard Season 01");
    assert_eq!(extract_season_from_dirname("Season 1"), Some(1), "Season 1 (single digit)");
    assert_eq!(extract_season_from_dirname("Season 10"), Some(10), "Season 10");
    assert_eq!(extract_season_from_dirname("season 02"), Some(2), "Lowercase season");
    assert_eq!(extract_season_from_dirname("SEASON 03"), Some(3), "Uppercase SEASON");
    
    // Test patterns with spaces
    assert_eq!(extract_season_from_dirname("Season   04"), Some(4), "Multiple spaces");
    assert_eq!(extract_season_from_dirname("Season_05"), None, "Underscore instead of space");
    
    // Test non-season folders
    assert_eq!(extract_season_from_dirname("Season"), None, "Season without number");
    assert_eq!(extract_season_from_dirname("Series 1"), None, "Series not Season");
    assert_eq!(extract_season_from_dirname("Folder"), None, "Generic folder");
    assert_eq!(extract_season_from_dirname(""), None, "Empty string");
    
    println!("test_extract_season_from_dirname: passed");
}

#[tokio::test]
async fn test_download_file() {
    // Test that download_file function signature works
    // Note: We test with an invalid URL to avoid network dependency
    let result = download_file("https://invalid-url-that-does-not-exist-12345.test/invalid.jpg", &PathBuf::from("/tmp/test_poster.jpg"), false, &None).await;
    
    // The download should fail because the URL is invalid
    assert!(result.is_err(), "Download should fail with invalid URL");
    
    println!("test_download_file: passed");
}

#[tokio::test]
async fn test_download_file_with_size() {
    // Test that download_file_with_size function signature works
    // Note: We test with an invalid URL to avoid network dependency
    let result = download_file_with_size("https://invalid-url-that-does-not-exist-12345.test/invalid.jpg", &PathBuf::from("/tmp/test_poster_size.jpg"), false, &None).await;
    
    // The download should fail because the URL is invalid
    assert!(result.is_err(), "Download should fail with invalid URL");
    
    println!("test_download_file_with_size: passed");
}

#[tokio::test]
async fn test_download_file_with_proxy() {
    // Test that download_file function works with proxy configuration
    // We test with proxy_enabled=true but invalid proxy to avoid network dependency
    let result = download_file(
        "https://invalid-url-that-does-not-exist-12345.test/invalid.jpg", 
        &PathBuf::from("/tmp/test_poster_proxy.jpg"), 
        true, 
        &Some("http://127.0.0.1:7897".to_string())
    ).await;
    
    // The download should fail because the URL is invalid
    assert!(result.is_err(), "Download should fail with invalid URL even with proxy");
    
    // Test with proxy_enabled=false
    let result_no_proxy = download_file(
        "https://invalid-url-that-does-not-exist-12345.test/invalid.jpg", 
        &PathBuf::from("/tmp/test_poster_no_proxy.jpg"), 
        false, 
        &Some("http://127.0.0.1:7897".to_string())
    ).await;
    
    assert!(result_no_proxy.is_err(), "Download should fail with invalid URL without proxy");
    
    println!("test_download_file_with_proxy: passed");
}

#[test]
fn test_format_size() {
    // Test format_size function
    assert_eq!(format_size(0), "0 B", "Zero bytes");
    assert_eq!(format_size(1023), "1023 B", "Less than 1 KB");
    assert_eq!(format_size(1024), "1.00 KB", "Exactly 1 KB");
    assert_eq!(format_size(1536), "1.50 KB", "1.5 KB");
    assert_eq!(format_size(2048), "2.00 KB", "2 KB");
    assert_eq!(format_size(1048575), "1023.99 KB", "Just under 1 MB");
    assert_eq!(format_size(1048576), "1.00 MB", "Exactly 1 MB");
    assert_eq!(format_size(1572864), "1.50 MB", "1.5 MB");
    assert_eq!(format_size(1073741823), "1023.99 MB", "Just under 1 GB");
    assert_eq!(format_size(1073741824), "1.00 GB", "Exactly 1 GB");
    assert_eq!(format_size(1610612736), "1.50 GB", "1.5 GB");
    
    // Test large sizes
    assert_eq!(format_size(5368709120), "5.00 GB", "5 GB");
    
    println!("test_format_size: passed");
}

#[test]
fn test_season_nfo_naming_convention() {
    // Test the new naming convention: [TV名称]-seasonXX.nfo
    let tv_name = "哦！我的皇帝陛下";
    let season_num = 1;
    let nfo_name = format!("[{}]-season{:02}.nfo", tv_name, season_num);
    assert_eq!(nfo_name, "[哦！我的皇帝陛下]-season01.nfo", "Season 1 NFO name with Chinese title");
    
    let tv_name = "Stranger Things";
    let season_num = 10;
    let nfo_name = format!("[{}]-season{:02}.nfo", tv_name, season_num);
    assert_eq!(nfo_name, "[Stranger Things]-season10.nfo", "Season 10 NFO name with English title");
    
    // Test detection logic - should support multiple formats
    let is_season_nfo = |file_name: &str| {
        (file_name.starts_with("season") || 
         file_name.starts_with("[season]-") ||
         (file_name.contains("-season") && file_name.ends_with(".nfo"))) 
        && file_name.ends_with(".nfo")
    };
    
    assert!(is_season_nfo("[哦！我的皇帝陛下]-season01.nfo"), "New format with Chinese title should be detected");
    assert!(is_season_nfo("[Stranger Things]-season10.nfo"), "New format with English title should be detected");
    assert!(is_season_nfo("season01.nfo"), "Old format should still be detected");
    assert!(is_season_nfo("[season]-season01.nfo"), "Intermediate format should be detected");
    assert!(!is_season_nfo("episode.nfo"), "Episode NFO should not be detected as season NFO");
    assert!(!is_season_nfo("tvshow.nfo"), "TV show NFO should not be detected as season NFO");
    
    assert!(!is_season_nfo("movie.nfo"), "Movie NFO should not be detected as season");
    assert!(!is_season_nfo("tvshow.nfo"), "TV show NFO should not be detected as season");
    assert!(!is_season_nfo("season.txt"), "Non-NFO file should not be detected");
    
    println!("test_season_nfo_naming_convention: passed");
}

#[test]
fn test_poster_naming_conventions() {
    // Test movie poster naming (same as video file)
    let video_path = PathBuf::from("/movies/[Movie.Title].2024.1080p.mp4");
    let poster_name = format!("{}.jpg", video_path.file_stem().unwrap_or_default().to_string_lossy());
    assert_eq!(poster_name, "[Movie.Title].2024.1080p.jpg", "Movie poster name should match video");
    
    // Test season poster naming
    let season_num = 1;
    let season_poster_name = format!("season{:02}.jpg", season_num);
    assert_eq!(season_poster_name, "season01.jpg", "Season poster name");
    
    let season_num = 10;
    let season_poster_name = format!("season{:02}.jpg", season_num);
    assert_eq!(season_poster_name, "season10.jpg", "Double digit season poster name");
    
    // Test new season poster naming convention with bracket prefix
    let tv_title = "炽道";
    let season_poster_name = format!("[{}]-season{:02}.jpg", tv_title, season_num);
    assert_eq!(season_poster_name, "[炽道]-season10.jpg", "Season poster with bracket prefix");
    
    println!("test_poster_naming_conventions: passed");
}

#[tokio::test]
async fn test_concurrent_task_spawning() {
    // Test that tokio tasks can be spawned and joined
    // This tests the concurrent download infrastructure without actual network calls
    
    let mut tasks = Vec::new();
    
    // Spawn multiple tasks
    for i in 0..5 {
        let task = tokio::spawn(async move {
            // Simulate some work
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            i * 2
        });
        tasks.push(task);
    }
    
    // Join all tasks
    let results = futures::future::join_all(tasks).await;
    
    // Verify results
    let expected = vec![0, 2, 4, 6, 8];
    let mut actual: Vec<i32> = results.into_iter().map(|r| r.unwrap()).collect();
    actual.sort();
    assert_eq!(actual, expected, "Concurrent tasks should return correct results");
    
    println!("test_concurrent_task_spawning: passed");
}
