//! Locale utilities for language and country code conversion.
//!
//! Provides functions to convert ISO 639-1 language codes and ISO 3166-1 country codes
//! to human-readable names. Used for folder naming, NFO generation, and metadata display.

/// Find Chinese title with priority order: CN > SG > HK > TW
/// Falls back to any available Chinese translation if no priority region is found
pub fn find_priority_chinese_title(candidates: &[(String, String)]) -> Option<String> {
    let region_priority = ["CN", "SG", "HK", "TW"];
    
    // First pass: try in priority order
    for priority_region in &region_priority {
        if let Some((_region, title)) = candidates.iter().find(|(r, _)| r == priority_region) {
            return Some(title.clone());
        }
    }
    
    // Final fallback: use any available Chinese translation
    candidates.first().map(|(_, title)| title.clone())
}

/// Convert ISO 3166-1 country code to human-readable name.
/// Used for metadata (countries field in NFO), NOT for folder classification.
pub fn country_code_to_name(code: &str) -> String {
    match code.to_uppercase().as_str() {
        "US" => "United States".to_string(),
        "GB" => "United Kingdom".to_string(),
        "CA" => "Canada".to_string(),
        "CN" => "China".to_string(),
        "JP" => "Japan".to_string(),
        "KR" => "South Korea".to_string(),
        "TW" => "Taiwan".to_string(),
        "HK" => "Hong Kong".to_string(),
        "FR" => "France".to_string(),
        "DE" => "Germany".to_string(),
        "ES" => "Spain".to_string(),
        "IT" => "Italy".to_string(),
        "AU" => "Australia".to_string(),
        "NZ" => "New Zealand".to_string(),
        "IN" => "India".to_string(),
        "TH" => "Thailand".to_string(),
        "ID" => "Indonesia".to_string(),
        "BR" => "Brazil".to_string(),
        "MX" => "Mexico".to_string(),
        "RU" => "Russia".to_string(),
        "NL" => "Netherlands".to_string(),
        "SE" => "Sweden".to_string(),
        "NO" => "Norway".to_string(),
        "DK" => "Denmark".to_string(),
        _ => code.to_uppercase(),
    }
}

/// Convert ISO 639-1 language code to human-readable name.
/// Used for folder naming: e.g., "zh" -> "Chinese" -> "ZH_Chinese"
pub fn language_code_to_name(code: &str) -> String {
    match code.to_lowercase().as_str() {
        // Major languages
        "en" => "English".to_string(),
        "zh" => "Chinese".to_string(),
        "ja" => "Japanese".to_string(),
        "ko" => "Korean".to_string(),
        "fr" => "French".to_string(),
        "de" => "German".to_string(),
        "es" => "Spanish".to_string(),
        "it" => "Italian".to_string(),
        "pt" => "Portuguese".to_string(),
        "ru" => "Russian".to_string(),
        // Asian languages
        "th" => "Thai".to_string(),
        "vi" => "Vietnamese".to_string(),
        "id" => "Indonesian".to_string(),
        "ms" => "Malay".to_string(),
        "tl" => "Filipino".to_string(),
        "hi" => "Hindi".to_string(),
        "ta" => "Tamil".to_string(),
        "te" => "Telugu".to_string(),
        "bn" => "Bengali".to_string(),
        // European languages
        "nl" => "Dutch".to_string(),
        "pl" => "Polish".to_string(),
        "sv" => "Swedish".to_string(),
        "no" => "Norwegian".to_string(),
        "da" => "Danish".to_string(),
        "fi" => "Finnish".to_string(),
        "cs" => "Czech".to_string(),
        "hu" => "Hungarian".to_string(),
        "el" => "Greek".to_string(),
        "tr" => "Turkish".to_string(),
        "uk" => "Ukrainian".to_string(),
        "ro" => "Romanian".to_string(),
        // Middle Eastern
        "ar" => "Arabic".to_string(),
        "he" => "Hebrew".to_string(),
        "fa" => "Persian".to_string(),
        // Chinese variants (TMDB sometimes uses these)
        "cn" => "Chinese".to_string(),
        "yue" => "Cantonese".to_string(),
        // Fallback
        _ => code.to_uppercase(),
    }
}

/// Normalize language code to standard ISO 639-1.
/// Handles TMDB quirks like "cn" -> "zh".
pub fn normalize_language_code(code: &str) -> &str {
    match code.to_lowercase().as_str() {
        "cn" => "zh",  // TMDB sometimes uses "cn" for Chinese
        _ => code,
    }
}

/// Format language folder name from original_language.
/// Returns format like "ZH_Chinese", "EN_English", etc.
pub fn format_language_folder(original_language: &str) -> String {
    let normalized = normalize_language_code(original_language);
    let name = language_code_to_name(normalized);
    format!("{}_{}", normalized.to_uppercase(), name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_code_to_name() {
        // Major languages
        assert_eq!(language_code_to_name("en"), "English");
        assert_eq!(language_code_to_name("zh"), "Chinese");
        assert_eq!(language_code_to_name("ja"), "Japanese");
        assert_eq!(language_code_to_name("ko"), "Korean");
        assert_eq!(language_code_to_name("fr"), "French");
        assert_eq!(language_code_to_name("de"), "German");
        assert_eq!(language_code_to_name("es"), "Spanish");
        assert_eq!(language_code_to_name("it"), "Italian");

        // Case insensitive
        assert_eq!(language_code_to_name("EN"), "English");
        assert_eq!(language_code_to_name("ZH"), "Chinese");

        // Asian languages
        assert_eq!(language_code_to_name("th"), "Thai");
        assert_eq!(language_code_to_name("vi"), "Vietnamese");
        assert_eq!(language_code_to_name("id"), "Indonesian");

        // Chinese variants
        assert_eq!(language_code_to_name("cn"), "Chinese");
        assert_eq!(language_code_to_name("yue"), "Cantonese");

        // Fallback
        assert_eq!(language_code_to_name("xx"), "XX");
        assert_eq!(language_code_to_name("unknown"), "UNKNOWN");
    }

    #[test]
    fn test_format_language_folder() {
        // Major languages
        assert_eq!(format_language_folder("en"), "EN_English");
        assert_eq!(format_language_folder("zh"), "ZH_Chinese");
        assert_eq!(format_language_folder("ja"), "JA_Japanese");
        assert_eq!(format_language_folder("ko"), "KO_Korean");
        assert_eq!(format_language_folder("fr"), "FR_French");

        // Case insensitive
        assert_eq!(format_language_folder("EN"), "EN_English");
        assert_eq!(format_language_folder("ZH"), "ZH_Chinese");

        // TMDB quirks: "cn" -> "zh"
        assert_eq!(format_language_folder("cn"), "ZH_Chinese");
        assert_eq!(format_language_folder("CN"), "ZH_Chinese");

        // Fallback
        assert_eq!(format_language_folder("xx"), "XX_XX");
    }

    #[test]
    fn test_normalize_language_code() {
        // TMDB quirks
        assert_eq!(normalize_language_code("cn"), "zh");
        assert_eq!(normalize_language_code("CN"), "zh");

        // Standard codes unchanged
        assert_eq!(normalize_language_code("zh"), "zh");
        assert_eq!(normalize_language_code("en"), "en");
        assert_eq!(normalize_language_code("ja"), "ja");
    }

    #[test]
    fn test_country_code_to_name() {
        // Major countries
        assert_eq!(country_code_to_name("US"), "United States");
        assert_eq!(country_code_to_name("CN"), "China");
        assert_eq!(country_code_to_name("JP"), "Japan");
        assert_eq!(country_code_to_name("KR"), "South Korea");
        assert_eq!(country_code_to_name("GB"), "United Kingdom");
        assert_eq!(country_code_to_name("ID"), "Indonesia");

        // Case insensitive
        assert_eq!(country_code_to_name("us"), "United States");
        assert_eq!(country_code_to_name("cn"), "China");

        // Fallback
        assert_eq!(country_code_to_name("XX"), "XX");
    }
}