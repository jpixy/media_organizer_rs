use media_organizer::core::planner::Planner;
use media_organizer::models::media::{VideoFile, VideoMetadata};
use media_organizer::models::plan::{Operation, OperationType, PlanItem, PlanItemStatus};
use media_organizer::services::tmdb::{MovieTranslations, TvTranslations};
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
fn test_find_priority_chinese_title_priority_order() {
    // Test priority order: CN > SG > HK > TW
    let candidates = vec![
        ("TW".to_string(), "繁體中文".to_string()),
        ("HK".to_string(), "香港繁體".to_string()),
        ("SG".to_string(), "简体中文(SG)".to_string()),
        ("CN".to_string(), "简体中文".to_string()),
    ];
    
    let result = media_organizer::utils::locale::find_priority_chinese_title(&candidates);
    
    // Should select CN first
    assert_eq!(result, Some("简体中文".to_string()));
}

#[test]
fn test_find_priority_chinese_title_fallback() {
    // Test fallback when no priority region is available
    let candidates = vec![
        ("JP".to_string(), "日本語".to_string()),
        ("KR".to_string(), "한국어".to_string()),
    ];
    
    let result = media_organizer::utils::locale::find_priority_chinese_title(&candidates);
    
    // Should fallback to first available
    assert_eq!(result, Some("日本語".to_string()));
}

#[test]
fn test_find_priority_chinese_title_empty() {
    // Test with empty candidates
    let candidates: Vec<(String, String)> = vec![];
    
    let result = media_organizer::utils::locale::find_priority_chinese_title(&candidates);
    
    // Should return None
    assert_eq!(result, None);
}

#[test]
fn test_find_priority_chinese_title_partial_priority() {
    // Test when only some priority regions are available
    let candidates = vec![
        ("HK".to_string(), "香港繁體".to_string()),
        ("TW".to_string(), "繁體中文".to_string()),
    ];
    
    let result = media_organizer::utils::locale::find_priority_chinese_title(&candidates);
    
    // Should select HK (higher priority than TW)
    assert_eq!(result, Some("香港繁體".to_string()));
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

#[test]
fn test_movie_chinese_translation_priority() {
    // Test that Movies use correct priority order: CN > SG > HK > TW
    let json_response = r#"{
        "id": 497698,
        "translations": [
            {
                "iso_3166_1": "TW",
                "iso_639_1": "zh",
                "name": "繁體中文",
                "english_name": "Mandarin",
                "data": {
                    "title": "黑寡婦",
                    "overview": "測試概述",
                    "homepage": ""
                }
            },
            {
                "iso_3166_1": "HK",
                "iso_639_1": "zh",
                "name": "繁體中文",
                "english_name": "Mandarin",
                "data": {
                    "title": "黑寡婦",
                    "overview": "測試概述",
                    "homepage": ""
                }
            },
            {
                "iso_3166_1": "SG",
                "iso_639_1": "zh",
                "name": "简体中文",
                "english_name": "Mandarin",
                "data": {
                    "title": "黑寡妇",
                    "overview": "测试概述",
                    "homepage": ""
                }
            },
            {
                "iso_3166_1": "CN",
                "iso_639_1": "zh",
                "name": "简体中文",
                "english_name": "Mandarin",
                "data": {
                    "title": "黑寡妇",
                    "overview": "测试概述",
                    "homepage": ""
                }
            }
        ]
    }"#;
    
    let translations: MovieTranslations = serde_json::from_str(json_response).unwrap();
    
    // Collect all valid Chinese translations
    let chinese_candidates: Vec<(String, String)> = translations.translations
        .iter()
        .filter(|t| t.iso_639_1 == "zh" || t.iso_639_1 == "zh-CN")
        .filter(|t| t.data.get_title().map_or(false, |s| !s.is_empty()))
        .map(|t| (t.iso_3166_1.clone(), t.data.get_title().unwrap_or_default().to_string()))
        .collect();
    
    // Priority order: CN > SG > HK > TW
    let region_priority = ["CN", "SG", "HK", "TW"];
    let mut selected_title = None;
    
    for priority_region in &region_priority {
        if let Some((_region, chinese_title)) = chinese_candidates
            .iter()
            .find(|(r, _)| r == priority_region)
        {
            selected_title = Some(chinese_title.clone());
            break;
        }
    }
    
    // Should select CN translation
    assert_eq!(selected_title, Some("黑寡妇".to_string()));
    assert_eq!(chinese_candidates.len(), 4);
}

#[test]
fn test_movie_chinese_translation_fallback() {
    // Test that Movies use fallback when no priority region is available
    let json_response = r#"{
        "id": 497698,
        "translations": [
            {
                "iso_3166_1": "TW",
                "iso_639_1": "zh",
                "name": "繁體中文",
                "english_name": "Mandarin",
                "data": {
                    "title": "黑寡婦",
                    "overview": "測試概述",
                    "homepage": ""
                }
            },
            {
                "iso_3166_1": "HK",
                "iso_639_1": "zh",
                "name": "繁體中文",
                "english_name": "Mandarin",
                "data": {
                    "title": "黑寡婦",
                    "overview": "測試概述",
                    "homepage": ""
                }
            }
        ]
    }"#;
    
    let translations: MovieTranslations = serde_json::from_str(json_response).unwrap();
    
    // Collect all valid Chinese translations
    let chinese_candidates: Vec<(String, String)> = translations.translations
        .iter()
        .filter(|t| t.iso_639_1 == "zh" || t.iso_639_1 == "zh-CN")
        .filter(|t| t.data.get_title().map_or(false, |s| !s.is_empty()))
        .map(|t| (t.iso_3166_1.clone(), t.data.get_title().unwrap_or_default().to_string()))
        .collect();
    
    // Priority order: CN > SG > HK > TW
    let region_priority = ["CN", "SG", "HK", "TW"];
    let mut selected_title = None;
    
    for priority_region in &region_priority {
        if let Some((_region, chinese_title)) = chinese_candidates
            .iter()
            .find(|(r, _)| r == priority_region)
        {
            selected_title = Some(chinese_title.clone());
            break;
        }
    }
    
    // No priority region found, should use fallback
    if selected_title.is_none() {
        if let Some((_region, chinese_title)) = chinese_candidates.first() {
            selected_title = Some(chinese_title.clone());
        }
    }
    
    // Should fallback to first available (HK in this case)
    assert_eq!(selected_title, Some("黑寡婦".to_string()));
    assert_eq!(chinese_candidates.len(), 2);
}

#[test]
fn test_tv_chinese_translation_priority() {
    // Test that TV shows use correct priority order: CN > SG > HK > TW
    let json_response = r#"{
        "id": 86831,
        "translations": [
            {
                "iso_3166_1": "TW",
                "iso_639_1": "zh",
                "name": "繁體中文",
                "english_name": "Mandarin",
                "data": {
                    "name": "愛 x 死 x 機器人",
                    "overview": "測試概述",
                    "homepage": ""
                }
            },
            {
                "iso_3166_1": "HK",
                "iso_639_1": "zh",
                "name": "繁體中文",
                "english_name": "Mandarin",
                "data": {
                    "name": "愛．死．機械人",
                    "overview": "測試概述",
                    "homepage": ""
                }
            },
            {
                "iso_3166_1": "SG",
                "iso_639_1": "zh",
                "name": "简体中文",
                "english_name": "Mandarin",
                "data": {
                    "name": "爱、死亡 & 机器人",
                    "overview": "测试概述",
                    "homepage": ""
                }
            },
            {
                "iso_3166_1": "CN",
                "iso_639_1": "zh",
                "name": "简体中文",
                "english_name": "Mandarin",
                "data": {
                    "name": "爱、死亡 & 机器人",
                    "overview": "测试概述",
                    "homepage": ""
                }
            }
        ]
    }"#;
    
    let translations: TvTranslations = serde_json::from_str(json_response).unwrap();
    
    // Collect all valid Chinese translations
    let chinese_candidates: Vec<(String, String)> = translations.translations
        .iter()
        .filter(|t| t.iso_639_1 == "zh" || t.iso_639_1 == "zh-CN")
        .filter(|t| t.data.get_title().map_or(false, |s| !s.is_empty()))
        .map(|t| (t.iso_3166_1.clone(), t.data.get_title().unwrap_or_default().to_string()))
        .collect();
    
    // Priority order: CN > SG > HK > TW
    let region_priority = ["CN", "SG", "HK", "TW"];
    let mut selected_title = None;
    
    for priority_region in &region_priority {
        if let Some((_region, chinese_title)) = chinese_candidates
            .iter()
            .find(|(r, _)| r == priority_region)
        {
            selected_title = Some(chinese_title.clone());
            break;
        }
    }
    
    // Should select CN translation
    assert_eq!(selected_title, Some("爱、死亡 & 机器人".to_string()));
    assert_eq!(chinese_candidates.len(), 4);
}

#[test]
fn test_tv_chinese_translation_fallback() {
    // Test that TV shows use fallback when no priority region is available
    let json_response = r#"{
        "id": 86831,
        "translations": [
            {
                "iso_3166_1": "TW",
                "iso_639_1": "zh",
                "name": "繁體中文",
                "english_name": "Mandarin",
                "data": {
                    "name": "愛 x 死 x 機器人",
                    "overview": "測試概述",
                    "homepage": ""
                }
            },
            {
                "iso_3166_1": "HK",
                "iso_639_1": "zh",
                "name": "繁體中文",
                "english_name": "Mandarin",
                "data": {
                    "name": "愛．死．機械人",
                    "overview": "測試概述",
                    "homepage": ""
                }
            }
        ]
    }"#;
    
    let translations: TvTranslations = serde_json::from_str(json_response).unwrap();
    
    // Collect all valid Chinese translations
    let chinese_candidates: Vec<(String, String)> = translations.translations
        .iter()
        .filter(|t| t.iso_639_1 == "zh" || t.iso_639_1 == "zh-CN")
        .filter(|t| t.data.get_title().map_or(false, |s| !s.is_empty()))
        .map(|t| (t.iso_3166_1.clone(), t.data.get_title().unwrap_or_default().to_string()))
        .collect();
    
    // Priority order: CN > SG > HK > TW
    let region_priority = ["CN", "SG", "HK", "TW"];
    let mut selected_title = None;
    
    for priority_region in &region_priority {
        if let Some((_region, chinese_title)) = chinese_candidates
            .iter()
            .find(|(r, _)| r == priority_region)
        {
            selected_title = Some(chinese_title.clone());
            break;
        }
    }
    
    // No priority region found, should use fallback
    if selected_title.is_none() {
        if let Some((_region, chinese_title)) = chinese_candidates.first() {
            selected_title = Some(chinese_title.clone());
        }
    }
    
    // Should fallback to first available (HK in this case, since it appears first in the collected candidates)
    assert_eq!(selected_title, Some("愛．死．機械人".to_string()));
    assert_eq!(chinese_candidates.len(), 2);
}

#[test]
fn test_movies_and_tv_translation_logic_consistency() {
    // Test that Movies and TV shows use the same translation logic
    
    // Movie data (uses "title" field)
    let movie_json = r#"{
        "id": 497698,
        "translations": [
            {
                "iso_3166_1": "CN",
                "iso_639_1": "zh",
                "name": "简体中文",
                "english_name": "Mandarin",
                "data": {
                    "title": "黑寡妇",
                    "overview": "测试概述",
                    "homepage": ""
                }
            },
            {
                "iso_3166_1": "TW",
                "iso_639_1": "zh",
                "name": "繁體中文",
                "english_name": "Mandarin",
                "data": {
                    "title": "黑寡婦",
                    "overview": "測試概述",
                    "homepage": ""
                }
            }
        ]
    }"#;
    
    // TV data (uses "name" field)
    let tv_json = r#"{
        "id": 86831,
        "translations": [
            {
                "iso_3166_1": "CN",
                "iso_639_1": "zh",
                "name": "简体中文",
                "english_name": "Mandarin",
                "data": {
                    "name": "爱、死亡 & 机器人",
                    "overview": "测试概述",
                    "homepage": ""
                }
            },
            {
                "iso_3166_1": "TW",
                "iso_639_1": "zh",
                "name": "繁體中文",
                "english_name": "Mandarin",
                "data": {
                    "name": "愛 x 死 x 機器人",
                    "overview": "測試概述",
                    "homepage": ""
                }
            }
        ]
    }"#;
    
    let movie_translations: MovieTranslations = serde_json::from_str(movie_json).unwrap();
    let tv_translations: TvTranslations = serde_json::from_str(tv_json).unwrap();
    
    // Process Movies
    let movie_candidates: Vec<(String, String)> = movie_translations.translations
        .iter()
        .filter(|t| t.iso_639_1 == "zh" || t.iso_639_1 == "zh-CN")
        .filter(|t| t.data.get_title().map_or(false, |s| !s.is_empty()))
        .map(|t| (t.iso_3166_1.clone(), t.data.get_title().unwrap_or_default().to_string()))
        .collect();
    
    // Process TV
    let tv_candidates: Vec<(String, String)> = tv_translations.translations
        .iter()
        .filter(|t| t.iso_639_1 == "zh" || t.iso_639_1 == "zh-CN")
        .filter(|t| t.data.get_title().map_or(false, |s| !s.is_empty()))
        .map(|t| (t.iso_3166_1.clone(), t.data.get_title().unwrap_or_default().to_string()))
        .collect();
    
    // Both should use the same priority logic
    let region_priority = ["CN", "SG", "HK", "TW"];
    
    let movie_result = region_priority.iter()
        .find_map(|priority_region| {
            movie_candidates.iter()
                .find(|(r, _)| r == priority_region)
                .map(|(_, title)| title.clone())
        });
    
    let tv_result = region_priority.iter()
        .find_map(|priority_region| {
            tv_candidates.iter()
                .find(|(r, _)| r == priority_region)
                .map(|(_, title)| title.clone())
        });
    
    // Both should select CN translation
    assert_eq!(movie_result, Some("黑寡妇".to_string()));
    assert_eq!(tv_result, Some("爱、死亡 & 机器人".to_string()));
    
    // Both should have same number of candidates
    assert_eq!(movie_candidates.len(), 2);
    assert_eq!(tv_candidates.len(), 2);
}
