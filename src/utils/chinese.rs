//! Chinese text utilities.

use pinyin::ToPinyin;

/// Check if two strings are the same when normalized (handles Traditional/Simplified).
pub fn titles_equivalent(a: &str, b: &str) -> bool {
    // Basic normalization for now
    // TODO: Implement proper Traditional/Simplified Chinese conversion
    normalize(a) == normalize(b)
}

/// Normalize a string for comparison.
pub fn normalize(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// Check if a string contains Chinese characters.
pub fn contains_chinese(s: &str) -> bool {
    s.chars().any(is_chinese_char)
}

/// Check if a character is a Chinese character.
fn is_chinese_char(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}' |  // CJK Unified Ideographs
        '\u{3400}'..='\u{4DBF}' |  // CJK Unified Ideographs Extension A
        '\u{F900}'..='\u{FAFF}' |  // CJK Compatibility Ideographs
        '\u{20000}'..='\u{2A6DF}'  // CJK Unified Ideographs Extension B
    )
}

/// Get the first pinyin letter of a string, uppercase.
/// Skips leading non-Chinese characters (like quotes, spaces, etc.) to find the first Chinese character.
/// If no Chinese character is found, falls back to uppercase first character.
pub fn get_first_pinyin_letter(s: &str) -> char {
    let mut first_non_chinese_char: Option<char> = None;
    
    for c in s.chars() {
        if is_chinese_char(c) {
            if let Some(pinyin) = c.to_pinyin() {
                let pinyin_str = pinyin.plain();
                if let Some(first_pinyin_char) = pinyin_str.chars().next() {
                    return first_pinyin_char.to_ascii_uppercase();
                }
            }
            // Fallback to the Chinese character itself uppercase
            return c.to_ascii_uppercase();
        } else if first_non_chinese_char.is_none() {
            first_non_chinese_char = Some(c);
        }
    }
    
    // No Chinese character found, return first character uppercase or '?'
    first_non_chinese_char.map(|c| c.to_ascii_uppercase()).unwrap_or('?')
}

/// Test various Chinese characters for pinyin conversion.
/// Returns true if all test characters pass conversion.
#[cfg(test)]
fn test_pinyin_chars() -> bool {
    let test_cases = vec![
        ("囡", 'N'),
        ("赤", 'C'),
        ("青", 'Q'),
        ("阿", 'A'),
        ("霸", 'B'),
        ("电", 'D'),
        ("影", 'Y'),
        ("中", 'Z'),
        ("文", 'W'),
        ("好", 'H'),
        ("了", 'L'),
        ("的", 'D'),
        ("是", 'S'),
        ("不", 'B'),
        ("我", 'W'),
        ("你", 'N'),
        ("他", 'T'),
        ("一", 'Y'),
        ("二", 'E'),
        ("三", 'S'),
        ("四", 'S'),
        ("五", 'W'),
        ("六", 'L'),
        ("七", 'Q'),
        ("八", 'B'),
        ("九", 'J'),
        ("十", 'S'),
        ("裸", 'L'),
        ("特", 'T'),
        ("工", 'G'),
        ("道", 'D'),
        ("苔", 'T'),
        ("卧", 'W'),
        ("虎", 'H'),
        ("龙", 'L'),
        ("卧虎藏龙", 'W'),
        ("黑客帝国", 'H'),
        ("阿凡达", 'A'),
        ("霸王别姬", 'B'),
        ("卧虎藏龙是一部好电影", 'W'),
    ];
    
    let mut all_passed = true;
    for (s, expected) in test_cases {
        let result = get_first_pinyin_letter(s);
        if result != expected {
            eprintln!("Test failed: '{}' expected '{}', got '{}'", s, expected, result);
            all_passed = false;
        }
    }
    all_passed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_chinese() {
        assert!(contains_chinese("阿凡达"));
        assert!(contains_chinese("Avatar 阿凡达"));
        assert!(!contains_chinese("Avatar"));
        assert!(!contains_chinese("The Matrix"));
    }

    #[test]
    fn test_titles_equivalent() {
        assert!(titles_equivalent("Avatar", "avatar"));
        assert!(titles_equivalent("The Matrix", "the matrix"));
        assert!(!titles_equivalent("Avatar", "Titanic"));
    }

    #[test]
    fn test_get_first_pinyin_letter() {
        assert!(test_pinyin_chars());
    }

    #[test]
    fn test_get_first_pinyin_letter_edge_cases() {
        // Test empty string
        assert_eq!(get_first_pinyin_letter(""), '?');
        
        // Test non-Chinese strings
        assert_eq!(get_first_pinyin_letter("Avatar"), 'A');
        assert_eq!(get_first_pinyin_letter("The Matrix"), 'T');
        assert_eq!(get_first_pinyin_letter("123"), '1');
        assert_eq!(get_first_pinyin_letter("!@#"), '!');
        assert_eq!(get_first_pinyin_letter("オリハルコン"), 'オ');
        assert_eq!(get_first_pinyin_letter("한국어"), '한');
        
        // Test mixed content
        assert_eq!(get_first_pinyin_letter("阿凡达 Avatar"), 'A');
        assert_eq!(get_first_pinyin_letter("Avatar 阿凡达"), 'A');
        
        // Test strings with leading quotes or special characters
        assert_eq!(get_first_pinyin_letter("\"吃吃\"的爱"), 'C');
        assert_eq!(get_first_pinyin_letter("\"骗骗\"喜欢你"), 'P');
        assert_eq!(get_first_pinyin_letter("'阿凡达'"), 'A');
        assert_eq!(get_first_pinyin_letter("【英雄】"), 'Y');
        assert_eq!(get_first_pinyin_letter("《泰坦尼克号》"), 'T');
        assert_eq!(get_first_pinyin_letter("  卧虎藏龙"), 'W');
        assert_eq!(get_first_pinyin_letter("-黑客帝国"), 'H');
        
        // Test various punctuation marks
        assert_eq!(get_first_pinyin_letter("。英雄"), 'Y');
        assert_eq!(get_first_pinyin_letter("，阿凡达"), 'A');
        assert_eq!(get_first_pinyin_letter("！泰坦尼克号"), 'T');
        assert_eq!(get_first_pinyin_letter("？卧虎藏龙"), 'W');
        assert_eq!(get_first_pinyin_letter("、黑客帝国"), 'H');
        assert_eq!(get_first_pinyin_letter("；三国"), 'S');
        assert_eq!(get_first_pinyin_letter("：赤壁"), 'C');
        assert_eq!(get_first_pinyin_letter("（英雄）"), 'Y');
        assert_eq!(get_first_pinyin_letter("）阿凡达"), 'A');
        assert_eq!(get_first_pinyin_letter("——泰坦尼克号"), 'T');
        assert_eq!(get_first_pinyin_letter("……卧虎藏龙"), 'W');
        assert_eq!(get_first_pinyin_letter("“英雄”"), 'Y');
        assert_eq!(get_first_pinyin_letter("‘阿凡达’"), 'A');
        assert_eq!(get_first_pinyin_letter("『泰坦尼克号』"), 'T');
        assert_eq!(get_first_pinyin_letter("【卧虎藏龙】"), 'W');
        assert_eq!(get_first_pinyin_letter("《黑客帝国》"), 'H');
        assert_eq!(get_first_pinyin_letter("〈三国〉"), 'S');
        assert_eq!(get_first_pinyin_letter("「赤壁」"), 'C');
        assert_eq!(get_first_pinyin_letter("『英雄』"), 'Y');
        
        // Test multiple leading non-Chinese characters
        assert_eq!(get_first_pinyin_letter("\"\"\"英雄"), 'Y');
        assert_eq!(get_first_pinyin_letter("  -  阿凡达"), 'A');
        assert_eq!(get_first_pinyin_letter("123英雄"), 'Y');
        assert_eq!(get_first_pinyin_letter("!@#$%^&*()英雄"), 'Y');
    }
}
