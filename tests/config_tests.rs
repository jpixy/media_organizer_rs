//! 配置文件加载测试
//! 
//! 这些测试验证配置文件路径解析和加载逻辑的正确性。

use media_organizer::models::config::{test_config_path, Config};
use std::path::Path;
use toml;

#[test]
fn test_config_path_structure() {
    // 测试配置路径结构
    let base_path = Path::new("/tmp/test_config");
    let config_dir = test_config_path(base_path);
    
    assert_eq!(config_dir, base_path.join("mediaorganizer"));
    println!("test_config_path_structure: passed");
}

#[test]
fn test_toml_parsing_with_full_config() {
    // 测试完整配置文件的 TOML 解析
    let config_content = r#"
[organize]
download_posters = true
poster_size = "w500"
generate_nfo = true
generate_movie_nfo = true
generate_tv_episode_nfo = true
generate_tv_season_nfo = true

[tmdb]
api_key = "test_tmdb_key_123"
language = "zh-CN"

[network]
proxy_enabled = true
proxy = "http://127.0.0.1:7890"

[ollama]
enabled = false
host = "localhost"
port = 11434
model = "qwen2.5:7b"
timeout = 60
"#;
    
    let config: Config = toml::from_str(config_content).unwrap();
    
    // 验证配置值
    assert_eq!(config.organize.download_posters, true);
    assert_eq!(config.organize.poster_size, "w500");
    assert_eq!(config.organize.generate_nfo, true);
    assert_eq!(config.tmdb.api_key, Some("test_tmdb_key_123".to_string()));
    assert_eq!(config.tmdb.language, "zh-CN");
    assert_eq!(config.network.proxy_enabled, true);
    assert_eq!(config.network.proxy, Some("http://127.0.0.1:7890".to_string()));
    assert_eq!(config.ollama.enabled, false);
    assert_eq!(config.ollama.host, "localhost");
    assert_eq!(config.ollama.port, 11434);
    assert_eq!(config.ollama.model, "qwen2.5:7b");
    
    println!("test_toml_parsing_with_full_config: passed");
}

#[test]
fn test_config_directory_name_is_mediaorganizer() {
    // 确保配置目录名称是 mediaorganizer（不带下划线）
    let base_path = Path::new("/home/test");
    let config_dir = test_config_path(base_path);
    
    assert!(config_dir.ends_with("mediaorganizer"));
    assert!(!config_dir.ends_with("media_organizer"));
    
    println!("test_config_directory_name_is_mediaorganizer: passed");
}

#[test]
fn test_toml_parsing_with_invalid_content() {
    // 测试无效 TOML 内容的处理
    let invalid_toml = r#"
[tmdb
api_key = "test"
"#;
    
    let result: Result<Config, toml::de::Error> = toml::from_str(invalid_toml);
    
    assert!(result.is_err());
    println!("test_toml_parsing_with_invalid_content: passed");
}

#[test]
fn test_config_defaults() {
    // 测试配置默认值
    let config = Config::default();
    
    assert_eq!(config.organize.download_posters, true);
    assert_eq!(config.organize.poster_size, "w500");
    assert_eq!(config.organize.generate_nfo, true);
    assert_eq!(config.organize.generate_movie_nfo, true);
    assert_eq!(config.organize.generate_tv_episode_nfo, true);
    assert_eq!(config.organize.generate_tv_season_nfo, true);
    assert_eq!(config.tmdb.api_key, None);
    assert_eq!(config.tmdb.language, "zh-CN");
    assert_eq!(config.network.proxy_enabled, false);
    assert_eq!(config.network.proxy, None);
    assert_eq!(config.ollama.enabled, false);
    assert_eq!(config.ollama.host, "localhost");
    assert_eq!(config.ollama.port, 11434);
    assert_eq!(config.ollama.model, "qwen2.5:7b");
    assert_eq!(config.ollama.timeout, 60);
    
    println!("test_config_defaults: passed");
}

#[test]
fn test_partial_config_overrides_defaults() {
    // 测试部分配置覆盖默认值
    let config_content = r#"
[tmdb]
api_key = "my_api_key"
language = "zh-CN"

[organize]
download_posters = false
"#;
    
    let config: Config = toml::from_str(config_content).unwrap();
    
    // 验证覆盖的值
    assert_eq!(config.tmdb.api_key, Some("my_api_key".to_string()));
    assert_eq!(config.organize.download_posters, false);
    
    // 验证未覆盖的值保持默认值
    assert_eq!(config.organize.poster_size, "w500");
    assert_eq!(config.tmdb.language, "zh-CN");
    
    println!("test_partial_config_overrides_defaults: passed");
}

#[test]
fn test_tmdb_api_key_config() {
    // 测试 TMDB API key 配置
    let config_content = r#"
[tmdb]
api_key = "b04ce72868d0071b09650ab99df1d3d0"
language = "zh-CN"
"#;
    
    let config: Config = toml::from_str(config_content).unwrap();
    
    assert_eq!(config.tmdb.api_key, Some("b04ce72868d0071b09650ab99df1d3d0".to_string()));
    assert_eq!(config.tmdb.language, "zh-CN");
    
    println!("test_tmdb_api_key_config: passed");
}

#[test]
fn test_config_file_path_format() {
    // 测试配置文件路径格式
    let home_dir = Path::new("/home/user");
    let config_dir = test_config_path(home_dir);
    let config_path = config_dir.join("config.toml");
    
    // 路径应该是 /home/user/mediaorganizer/config.toml
    assert!(config_path.starts_with("/home/user"));
    assert!(config_path.parent().unwrap().ends_with("mediaorganizer"));
    assert_eq!(config_path.file_name().unwrap(), "config.toml");
    
    println!("test_config_file_path_format: passed");
}

#[test]
fn test_sessions_dir_path() {
    // 测试 sessions 目录路径
    let config = Config::default();
    
    // sessions 目录应该在配置目录下的 sessions 文件夹
    assert!(config.sessions_dir.ends_with("mediaorganizer/sessions"));
    
    println!("test_sessions_dir_path: passed");
}

#[test]
fn test_organize_config_defaults() {
    // 测试 organize 配置默认值
    let config = Config::default();
    
    assert_eq!(config.organize.download_posters, true);
    assert_eq!(config.organize.poster_size, "w500");
    assert_eq!(config.organize.generate_nfo, true);
    assert_eq!(config.organize.generate_movie_nfo, true);
    assert_eq!(config.organize.generate_tv_episode_nfo, true);
    assert_eq!(config.organize.generate_tv_season_nfo, true);
    
    println!("test_organize_config_defaults: passed");
}
