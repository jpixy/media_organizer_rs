use media_organizer::models::media::{SeasonMetadata, VideoFile, VideoMetadata};
use media_organizer::models::plan::{Operation, OperationType, ParsedInfo, PlanItem, PlanItemStatus, TargetInfo};
use std::path::PathBuf;
use uuid::Uuid;

#[test]
fn test_season_metadata_structure() {
    // Test that SeasonMetadata struct can be created with valid data
    let season_meta = SeasonMetadata {
        season_number: 1,
        name: "Season 1".to_string(),
        overview: Some("This is season 1".to_string()),
        air_date: Some("2023-01-01".to_string()),
        poster_url: Some("https://example.com/poster.jpg".to_string()),
        episode_count: 10,
        tmdb_id: 123456,
    };
    
    assert_eq!(season_meta.season_number, 1);
    assert_eq!(season_meta.name, "Season 1");
    assert!(season_meta.overview.is_some());
    assert!(season_meta.air_date.is_some());
    assert!(season_meta.poster_url.is_some());
    assert_eq!(season_meta.episode_count, 10);
    
    println!("Test passed: SeasonMetadata structure is valid!");
}

#[test]
fn test_plan_item_with_season_metadata() {
    // Test that PlanItem can hold season metadata
    let plan_item = create_test_plan_item(Some(SeasonMetadata {
        season_number: 2,
        name: "Season 2".to_string(),
        overview: Some("This is season 2".to_string()),
        air_date: None,
        poster_url: None,
        episode_count: 13,
        tmdb_id: 123457,
    }));
    
    assert!(plan_item.season_metadata.is_some());
    assert_eq!(plan_item.season_metadata.as_ref().unwrap().season_number, 2);
    assert_eq!(plan_item.season_metadata.as_ref().unwrap().name, "Season 2");
    assert_eq!(plan_item.season_metadata.as_ref().unwrap().episode_count, 13);
    
    println!("Test passed: PlanItem can hold season metadata!");
}

#[test]
fn test_plan_item_without_season_metadata() {
    // Test that PlanItem can exist without season metadata
    let plan_item = create_test_plan_item(None);
    
    assert!(plan_item.season_metadata.is_none());
    
    println!("Test passed: PlanItem can exist without season metadata!");
}

#[test]
fn test_season_metadata_with_none_values() {
    // Test SeasonMetadata with optional fields as None
    let season_meta = SeasonMetadata {
        season_number: 3,
        name: "Season 3".to_string(),
        overview: None,
        air_date: None,
        poster_url: None,
        episode_count: 8,
        tmdb_id: 123458,
    };
    
    assert_eq!(season_meta.season_number, 3);
    assert_eq!(season_meta.name, "Season 3");
    assert!(season_meta.overview.is_none());
    assert!(season_meta.air_date.is_none());
    assert!(season_meta.poster_url.is_none());
    assert_eq!(season_meta.episode_count, 8);
    
    println!("Test passed: SeasonMetadata works with None values!");
}

fn create_test_plan_item(season_meta: Option<SeasonMetadata>) -> PlanItem {
    PlanItem {
        id: Uuid::new_v4().to_string(),
        status: PlanItemStatus::Pending,
        source: VideoFile {
            path: PathBuf::from("/tmp/test.mp4"),
            filename: "test.mp4".to_string(),
            parent_dir: PathBuf::from("/tmp"),
            size: 1024,
            modified: chrono::Utc::now(),
            is_sample: false,
        },
        parsed: ParsedInfo {
            title: Some("Test Show".to_string()),
            original_title: None,
            year: Some(2023),
            confidence: 1.0,
            raw_response: None,
        },
        movie_metadata: None,
        tv_series_metadata: None,
        episode_metadata: None,
        season_metadata: season_meta,
        video_metadata: VideoMetadata::default(),
        target: TargetInfo {
            folder: "Season 01".to_string(),
            filename: "test.mp4".to_string(),
            full_path: PathBuf::from("/tmp/Season 01/test.mp4"),
            nfo: "test.nfo".to_string(),
            poster: None,
        },
        operations: vec![Operation {
            op: OperationType::Move,
            from: Some(PathBuf::from("/tmp/test.mp4")),
            to: PathBuf::from("/tmp/Season 01/test.mp4"),
            url: None,
            content_ref: None,
        }],
        poster_download: None,
    }
}
