use media_organizer::core::planner::Planner;
use media_organizer::models::media::{VideoFile, VideoMetadata};
use media_organizer::models::plan::{Operation, OperationType, PlanItem, PlanItemStatus};
use std::path::PathBuf;
use uuid::Uuid;

fn create_test_plan_item(path: &str, target_path: &str) -> PlanItem {
    PlanItem {
        id: Uuid::new_v4().to_string(),
        status: PlanItemStatus::Pending,
        source: VideoFile {
            path: PathBuf::from(path),
            filename: path.split('/').last().unwrap().to_string(),
            parent_dir: PathBuf::from(path.rsplit_once('/').unwrap().0),
            size: 1024,
            modified: chrono::Utc::now(),
            is_sample: false,
        },
        parsed: Default::default(),
        movie_metadata: None,
        tv_series_metadata: None,
        episode_metadata: None,
        season_metadata: None,
        video_metadata: VideoMetadata::default(),
        target: Default::default(),
        operations: vec![Operation {
            op: OperationType::Move,
            from: Some(PathBuf::from(path)),
            to: PathBuf::from(target_path),
            url: None,
            content_ref: None,
        }],
        poster_download: None,
    }
}

#[test]
fn test_validate_no_duplicate_targets() {
    let planner = Planner::new().unwrap();

    // 测试场景1: 两个文件有相同目标路径
    let mut items = vec![
        create_test_plan_item("/source/file1.mp4", "/target/file.mp4"),
        create_test_plan_item("/source/file2.mp4", "/target/file.mp4"),
    ];

    let result = planner.validate_no_duplicate_targets(&mut items);
    assert!(result.is_ok());

    // 验证第一个项目保留，第二个项目被标记为Skip
    assert_eq!(items[0].status, PlanItemStatus::Pending);
    assert!(!items[0].operations.is_empty());

    assert_eq!(items[1].status, PlanItemStatus::Skip);
    assert!(items[1].operations.is_empty());

    // 测试场景2: 三个文件有相同目标路径
    let mut items = vec![
        create_test_plan_item("/source/a.mp4", "/target/out.mp4"),
        create_test_plan_item("/source/b.mp4", "/target/out.mp4"),
        create_test_plan_item("/source/c.mp4", "/target/out.mp4"),
    ];

    let result = planner.validate_no_duplicate_targets(&mut items);
    assert!(result.is_ok());

    // 第一个保留，后面两个都应该被标记为Skip
    assert_eq!(items[0].status, PlanItemStatus::Pending);
    assert_eq!(items[1].status, PlanItemStatus::Skip);
    assert_eq!(items[2].status, PlanItemStatus::Skip);

    // 测试场景3: 多组重复
    let mut items = vec![
        create_test_plan_item("/source/1a.mp4", "/target/1.mp4"),
        create_test_plan_item("/source/1b.mp4", "/target/1.mp4"),
        create_test_plan_item("/source/2a.mp4", "/target/2.mp4"),
        create_test_plan_item("/source/2b.mp4", "/target/2.mp4"),
        create_test_plan_item("/source/unique.mp4", "/target/unique.mp4"),
    ];

    let result = planner.validate_no_duplicate_targets(&mut items);
    assert!(result.is_ok());

    assert_eq!(items[0].status, PlanItemStatus::Pending);
    assert_eq!(items[1].status, PlanItemStatus::Skip);
    assert_eq!(items[2].status, PlanItemStatus::Pending);
    assert_eq!(items[3].status, PlanItemStatus::Skip);
    assert_eq!(items[4].status, PlanItemStatus::Pending);

    // 测试场景4: 没有重复的情况
    let mut items = vec![
        create_test_plan_item("/source/a.mp4", "/target/a.mp4"),
        create_test_plan_item("/source/b.mp4", "/target/b.mp4"),
        create_test_plan_item("/source/c.mp4", "/target/c.mp4"),
    ];

    let result = planner.validate_no_duplicate_targets(&mut items);
    assert!(result.is_ok());

    // 所有项目都应该保持Pending状态
    for item in &items {
        assert_eq!(item.status, PlanItemStatus::Pending);
        assert!(!item.operations.is_empty());
    }
}

#[test]
fn test_validate_no_duplicate_targets_mixed_operations() {
    let planner = Planner::new().unwrap();

    // 测试只有Move操作会被检查重复
    let mut items = vec![
        PlanItem {
            operations: vec![
                Operation {
                    op: OperationType::Move,
                    from: Some(PathBuf::from("/source/file1.mp4")),
                    to: PathBuf::from("/target/file.mp4"),
                    url: None,
                    content_ref: None,
                },
                Operation {
                    op: OperationType::Create,
                    from: None,
                    to: PathBuf::from("/target/file.nfo"),
                    url: None,
                    content_ref: Some("nfo".to_string()),
                },
            ],
            ..create_test_plan_item("/source/file1.mp4", "/target/file.mp4")
        },
        PlanItem {
            operations: vec![
                Operation {
                    op: OperationType::Move,
                    from: Some(PathBuf::from("/source/file2.mp4")),
                    to: PathBuf::from("/target/file.mp4"),
                    url: None,
                    content_ref: None,
                },
                Operation {
                    op: OperationType::Create,
                    from: None,
                    to: PathBuf::from("/target/file.nfo"), // Create操作重复不会被视为冲突
                    url: None,
                    content_ref: Some("nfo".to_string()),
                },
            ],
            ..create_test_plan_item("/source/file2.mp4", "/target/file.mp4")
        },
    ];

    let result = planner.validate_no_duplicate_targets(&mut items);
    assert!(result.is_ok());

    // 只有Move操作的重复会导致第二个项目被标记为Skip
    assert_eq!(items[0].status, PlanItemStatus::Pending);
    assert_eq!(items[1].status, PlanItemStatus::Skip);
}