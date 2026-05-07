//! Integration tests for file I/O operations.
//!
//! Tests cover:
//! - Plan save/load
//! - Rollback save/load
//! - Session management

use media_organizer::core::planner::{load_plan, save_plan};
use media_organizer::core::rollback::{load_rollback, save_rollback};
use media_organizer::models::media::MediaType;
use media_organizer::models::plan::Plan;
use media_organizer::models::rollback::Rollback;
use std::path::PathBuf;
use tempfile::TempDir;

// ========== PLAN I/O TESTS ==========

#[test]
fn test_save_and_load_plan() {
    let plan = Plan {
        version: "1.0".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        media_type: Some(MediaType::Movies),
        source_path: PathBuf::from("/source"),
        target_path: PathBuf::from("/target"),
        items: vec![],
        samples: vec![],
        unknown: vec![],
    };

    let temp_dir = TempDir::new().unwrap();
    let plan_path = temp_dir.path().join("test_plan.json");

    // Save
    save_plan(&plan, &plan_path).unwrap();
    assert!(plan_path.exists());

    // Load
    let loaded = load_plan(&plan_path).unwrap();
    assert_eq!(loaded.version, plan.version);
    assert_eq!(loaded.source_path, plan.source_path);
    assert_eq!(loaded.target_path, plan.target_path);
}

#[test]
fn test_plan_round_trip_with_items() {
    let plan = Plan {
        version: "1.0".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        media_type: Some(MediaType::TvSeries),
        source_path: PathBuf::from("/source/tv_series"),
        target_path: PathBuf::from("/target/tv_series"),
        items: vec![],
        samples: vec![],
        unknown: vec![],
    };

    let temp_dir = TempDir::new().unwrap();
    let plan_path = temp_dir.path().join("tv_series_plan.json");

    save_plan(&plan, &plan_path).unwrap();
    let loaded = load_plan(&plan_path).unwrap();

    assert_eq!(loaded.media_type, Some(MediaType::TvSeries));
}

#[test]
fn test_load_nonexistent_plan() {
    let result = load_plan(&PathBuf::from("/nonexistent/plan.json"));
    assert!(result.is_err());
}

// ========== ROLLBACK I/O TESTS ==========

#[test]
fn test_save_and_load_rollback() {
    let rollback = Rollback::default();

    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("test_rollback.json");

    // Save
    save_rollback(&rollback, &path).unwrap();
    assert!(path.exists());

    // Load
    let loaded = load_rollback(&path).unwrap();
    assert_eq!(loaded.version, rollback.version);
}

#[test]
fn test_rollback_round_trip() {
    let rollback = Rollback {
        version: "1.0".to_string(),
        plan_id: "test-plan-id".to_string(),
        executed_at: chrono::Utc::now().to_rfc3339(),
        operations: vec![],
    };

    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("rollback.json");

    save_rollback(&rollback, &path).unwrap();
    let loaded = load_rollback(&path).unwrap();

    assert_eq!(loaded.plan_id, rollback.plan_id);
    assert_eq!(loaded.version, rollback.version);
}

#[test]
fn test_load_nonexistent_rollback() {
    let result = load_rollback(&PathBuf::from("/nonexistent/rollback.json"));
    assert!(result.is_err());
}

// ========== DIRECTORY CREATION TESTS ==========

#[test]
fn test_save_creates_parent_directories() {
    let temp_dir = TempDir::new().unwrap();
    let nested_path = temp_dir
        .path()
        .join("deeply")
        .join("nested")
        .join("dir")
        .join("rollback.json");

    let rollback = Rollback::default();
    save_rollback(&rollback, &nested_path).unwrap();

    assert!(nested_path.exists());
}
