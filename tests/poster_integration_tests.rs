//! Integration tests for poster download command.

use media_organizer::cli::commands::poster::{download_movie_posters, download_tv_season_posters};
use media_organizer::models::config::{Config, TmdbConfig};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[tokio::test]
async fn test_download_movie_posters_with_invalid_path() {
    let config = Config {
        tmdb: TmdbConfig {
            api_key: Some("test_key".to_string()),
            language: "zh-CN".to_string(),
        },
        ..Default::default()
    };
    
    let invalid_path = PathBuf::from("/nonexistent/path/that/does/not/exist");
    
    let result = download_movie_posters(&invalid_path, &config).await;
    
    assert!(result.is_err(), "Should return error for non-existent path");
    
    println!("test_download_movie_posters_with_invalid_path: passed");
}

#[tokio::test]
async fn test_download_tv_season_posters_with_invalid_path() {
    let config = Config {
        tmdb: TmdbConfig {
            api_key: Some("test_key".to_string()),
            language: "zh-CN".to_string(),
        },
        ..Default::default()
    };
    
    let invalid_path = PathBuf::from("/nonexistent/path/that/does/not/exist");
    
    let result = download_tv_season_posters(&invalid_path, &config).await;
    
    assert!(result.is_err(), "Should return error for non-existent path");
    
    println!("test_download_tv_season_posters_with_invalid_path: passed");
}

#[tokio::test]
async fn test_download_movie_posters_without_api_key() {
    let config = Config {
        tmdb: TmdbConfig {
            api_key: None,
            language: "zh-CN".to_string(),
        },
        ..Default::default()
    };
    
    let temp_dir = tempdir().unwrap();
    
    let result = download_movie_posters(temp_dir.path(), &config).await;
    
    assert!(result.is_err(), "Should return error when TMDB API key is not configured");
    assert!(result.unwrap_err().to_string().contains("TMDB API key not configured"));
    
    println!("test_download_movie_posters_without_api_key: passed");
}

#[tokio::test]
async fn test_download_tv_season_posters_without_api_key() {
    let config = Config {
        tmdb: TmdbConfig {
            api_key: None,
            language: "zh-CN".to_string(),
        },
        ..Default::default()
    };
    
    let temp_dir = tempdir().unwrap();
    
    let result = download_tv_season_posters(temp_dir.path(), &config).await;
    
    assert!(result.is_err(), "Should return error when TMDB API key is not configured");
    assert!(result.unwrap_err().to_string().contains("TMDB API key not configured"));
    
    println!("test_download_tv_season_posters_without_api_key: passed");
}

#[tokio::test]
async fn test_download_movie_posters_with_empty_dir() {
    let config = Config {
        tmdb: TmdbConfig {
            api_key: Some("test_key".to_string()),
            language: "zh-CN".to_string(),
        },
        ..Default::default()
    };
    
    let temp_dir = tempdir().unwrap();
    
    let result = download_movie_posters(temp_dir.path(), &config).await;
    
    assert!(result.is_ok(), "Should succeed with empty directory");
    
    println!("test_download_movie_posters_with_empty_dir: passed");
}

#[tokio::test]
async fn test_download_tv_season_posters_with_empty_dir() {
    let config = Config {
        tmdb: TmdbConfig {
            api_key: Some("test_key".to_string()),
            language: "zh-CN".to_string(),
        },
        ..Default::default()
    };
    
    let temp_dir = tempdir().unwrap();
    
    let result = download_tv_season_posters(temp_dir.path(), &config).await;
    
    assert!(result.is_ok(), "Should succeed with empty directory");
    
    println!("test_download_tv_season_posters_with_empty_dir: passed");
}

#[tokio::test]
async fn test_download_movie_posters_with_non_movie_folder() {
    let config = Config {
        tmdb: TmdbConfig {
            api_key: Some("test_key".to_string()),
            language: "zh-CN".to_string(),
        },
        ..Default::default()
    };
    
    let temp_dir = tempdir().unwrap();
    
    // Create a folder without movie.nfo
    let non_movie_folder = temp_dir.path().join("[Test Movie](2024)-tmdb123456");
    fs::create_dir_all(&non_movie_folder).unwrap();
    
    // Create a video file but no movie.nfo
    fs::write(non_movie_folder.join("movie.mp4"), b"test content").unwrap();
    
    let result = download_movie_posters(temp_dir.path(), &config).await;
    
    assert!(result.is_ok(), "Should succeed even with non-movie folders");
    
    println!("test_download_movie_posters_with_non_movie_folder: passed");
}

#[tokio::test]
async fn test_download_tv_season_posters_with_non_tv_folder() {
    let config = Config {
        tmdb: TmdbConfig {
            api_key: Some("test_key".to_string()),
            language: "zh-CN".to_string(),
        },
        ..Default::default()
    };
    
    let temp_dir = tempdir().unwrap();
    
    // Create a folder without tvshow.nfo
    let non_tv_folder = temp_dir.path().join("[Test Show](2024)-tmdb123456");
    fs::create_dir_all(&non_tv_folder).unwrap();
    
    let result = download_tv_season_posters(temp_dir.path(), &config).await;
    
    assert!(result.is_ok(), "Should succeed even with non-TV folders");
    
    println!("test_download_tv_season_posters_with_non_tv_folder: passed");
}

#[test]
fn test_poster_command_cli_structure() {
    use clap::CommandFactory;
    use media_organizer::cli::args::Cli;
    
    // Test that the CLI structure is correct
    let cli = Cli::command();
    
    // Verify poster command exists
    let poster_command = cli.find_subcommand("poster");
    assert!(poster_command.is_some(), "Poster command should exist");
    
    // Verify movies subcommand exists
    let movies_command = poster_command.unwrap().find_subcommand("movies");
    assert!(movies_command.is_some(), "Movies subcommand should exist");
    
    // Verify tv_series subcommand exists
    let tv_series_command = poster_command.unwrap().find_subcommand("tv_series");
    assert!(tv_series_command.is_some(), "TV series subcommand should exist");
    
    println!("test_poster_command_cli_structure: passed");
}
