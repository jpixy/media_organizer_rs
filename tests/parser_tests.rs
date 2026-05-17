use media_organizer::core::parser::FilenameParser;
use media_organizer::models::media::MediaType;

#[tokio::test]
async fn test_parse_mixed_chinese_english_filename() {
    let parser = FilenameParser::new();
    
    // Test parsing 爱妻物语.A.Beloved.Wife.2019.mp4
    let result = parser.parse("爱妻物语.A.Beloved.Wife.2019.mp4", MediaType::Movies).await.unwrap();
    assert_eq!(result.title, Some("爱妻物语".to_string()));
    assert_eq!(result.original_title, Some("A Beloved Wife".to_string()));
    assert_eq!(result.year, Some(2019));
    
    // Test parsing 人工情报.Humint.2026.mp4
    let result = parser.parse("人工情报.Humint.2026.mp4", MediaType::Movies).await.unwrap();
    assert_eq!(result.title, Some("人工情报".to_string()));
    assert_eq!(result.original_title, Some("Humint".to_string()));
    assert_eq!(result.year, Some(2026));
    
    // Test parsing 嚎叫.2012.mkv (pure Chinese)
    let result = parser.parse("嚎叫.2012.mkv", MediaType::Movies).await.unwrap();
    assert!(result.title.is_some(), "嚎叫.2012.mkv should parse with title");
    assert_eq!(result.year, Some(2012));
    
    // Test parsing 我的朋友安德烈.(2024).mp4
    let result = parser.parse("我的朋友安德烈.(2024).mp4", MediaType::Movies).await.unwrap();
    assert!(result.title.is_some(), "我的朋友安德烈.(2024).mp4 should parse with title");
    assert_eq!(result.year, Some(2024));
    
    // Test parsing 极限审判.2026.mp4
    let result = parser.parse("极限审判.2026.mp4", MediaType::Movies).await.unwrap();
    assert!(result.title.is_some(), "极限审判.2026.mp4 should parse with title");
    assert_eq!(result.year, Some(2026));
    
    // Test parsing 红杏出墙.2013.mp4
    let result = parser.parse("红杏出墙.2013.mp4", MediaType::Movies).await.unwrap();
    assert!(result.title.is_some(), "红杏出墙.2013.mp4 should parse with title");
    assert_eq!(result.year, Some(2013));
    
    // Test parsing 请求救援.2026.mp4
    let result = parser.parse("请求救援.2026.mp4", MediaType::Movies).await.unwrap();
    assert!(result.title.is_some(), "请求救援.2026.mp4 should parse with title");
    assert_eq!(result.year, Some(2026));
    
    // Test parsing 铁雨.mkv
    let result = parser.parse("铁雨.mkv", MediaType::Movies).await.unwrap();
    assert!(result.title.is_some(), "铁雨.mkv should parse with title");
}

#[tokio::test]
async fn test_parse_pure_english_filename() {
    let parser = FilenameParser::new();
    
    // Test pure English title - should be in original_title
    let result = parser.parse("Inception.2010.mp4", MediaType::Movies).await.unwrap();
    assert_eq!(result.original_title, Some("Inception".to_string()));
    assert_eq!(result.year, Some(2010));
    
    // Test shorter English title
    let result = parser.parse("Avatar.2009.mkv", MediaType::Movies).await.unwrap();
    assert_eq!(result.original_title, Some("Avatar".to_string()));
    assert_eq!(result.year, Some(2009));
}

#[tokio::test]
async fn test_parse_complex_filename() {
    let parser = FilenameParser::new();
    
    // Test with resolution and codec info - Chinese + English
    let result = parser.parse("流浪地球2.The.Wandering.Earth.2.2023.1080p.BluRay.x264.mp4", MediaType::Movies).await.unwrap();
    assert_eq!(result.title, Some("流浪地球2".to_string()));
    assert!(result.original_title.is_some(), "Should have original_title");
    assert_eq!(result.year, Some(2023));
    
    // Test with Chinese title only
    let result = parser.parse("哪吒之魔童降世.2019.mkv", MediaType::Movies).await.unwrap();
    assert!(result.title.is_some(), "哪吒之魔童降世.2019.mkv should parse with title");
    assert_eq!(result.year, Some(2019));
}

#[tokio::test]
async fn test_parse_with_alternative_title_array() {
    let parser = FilenameParser::new();
    
    // Test files that previously failed due to alternative_title being an array
    let result = parser.parse("Crime.101.2026.1080p中英字幕.mp4", MediaType::Movies).await;
    assert!(result.is_ok(), "Crime.101.2026.1080p中英字幕.mp4 should parse successfully");
    
    let result = parser.parse("The.Bluff.2026.4KHDR.官方中字.404.mp4", MediaType::Movies).await;
    assert!(result.is_ok(), "The.Bluff.2026.4KHDR.官方中字.404.mp4 should parse successfully");
}

#[tokio::test]
async fn test_parse_duplicate_files_scenario() {
    let parser = FilenameParser::new();
    
    // These are the exact files from the user's duplicate detection scenario
    let problematic_files = [
        "人工情报.Humint.2026.mp4",
        "嚎叫.2012.mkv", 
        "我的朋友安德烈.(2024).mp4",
        "极限审判.2026.mp4",
        "爱妻物语.A.Beloved.Wife.2019.mp4",
        "红杏出墙.2013.mp4",
        "请求救援.2026.mp4",
        "铁雨.mkv",
        "另有他路.mp4",
    ];
    
    for file in problematic_files.iter() {
        let result = parser.parse(file, MediaType::Movies).await;
        assert!(result.is_ok(), "Failed to parse: {}", file);
        
        let parsed = result.unwrap();
        assert!(parsed.title.is_some() || parsed.original_title.is_some(), 
                "File {} should have either title or original_title", file);
    }
}