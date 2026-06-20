use std::collections::HashSet;

pub struct TitleSimilarity {
    pub score: f64,
    pub matched: bool,
    pub reason: String,
}

pub fn normalize_title(title: &str) -> String {
    let mut result = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() || c.is_whitespace() {
            result.push(c.to_ascii_lowercase());
        }
    }
    result.split_whitespace().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

pub fn title_contains(a: &str, b: &str) -> bool {
    let a_norm = normalize_title(a);
    let b_norm = normalize_title(b);
    
    if a_norm.is_empty() || b_norm.is_empty() {
        return false;
    }
    
    let a_is_chinese = a_norm.chars().any(|c| c.is_ascii() == false);
    let b_is_chinese = b_norm.chars().any(|c| c.is_ascii() == false);
    
    if a_is_chinese || b_is_chinese {
        a_norm.contains(&b_norm) || b_norm.contains(&a_norm)
    } else {
        let a_words: HashSet<&str> = a_norm.split_whitespace().collect();
        let b_words: HashSet<&str> = b_norm.split_whitespace().collect();
        
        if a_words.is_empty() || b_words.is_empty() {
            return false;
        }
        
        let intersection: HashSet<_> = a_words.intersection(&b_words).collect();
        let min_len = std::cmp::min(a_words.len(), b_words.len()) as f64;
        
        (intersection.len() as f64 / min_len) >= 0.6
    }
}

pub fn compare_titles(
    parsed_title: &str,
    parsed_year: Option<u16>,
    api_title: &str,
    api_original_title: Option<&str>,
    api_year: Option<u16>,
) -> TitleSimilarity {
    let parsed_norm = normalize_title(parsed_title);
    
    if parsed_norm.is_empty() {
        return TitleSimilarity {
            score: 0.0,
            matched: false,
            reason: "解析标题为空".to_string(),
        };
    }
    
    let api_norm = normalize_title(api_title);
    let original_norm = api_original_title.map(|t| normalize_title(t));
    
    let title_matches = title_contains(&parsed_norm, &api_norm) || 
                       original_norm.as_ref().map_or(false, |o| title_contains(&parsed_norm, o)) ||
                       title_contains(&api_norm, &parsed_norm) ||
                       original_norm.as_ref().map_or(false, |o| title_contains(o, &parsed_norm));
    
    let year_matches = match (parsed_year, api_year) {
        (Some(py), Some(ay)) => (py as i32 - ay as i32).abs() <= 1,
        (None, _) | (_, None) => true,
    };
    
    let (score, matched, reason) = if title_matches && year_matches {
        let s = if title_contains(&parsed_norm, &api_norm) && title_contains(&api_norm, &parsed_norm) {
            1.0
        } else {
            0.75
        };
        (s, true, "标题和年份匹配".to_string())
    } else if title_matches {
        (0.6, true, "标题匹配但年份不匹配".to_string())
    } else {
        (0.0, false, "标题不匹配".to_string())
    };
    
    TitleSimilarity { score, matched, reason }
}

pub fn is_valid_imdb_id(s: &str) -> bool {
    s.starts_with("tt") && s[2..].chars().all(|c| c.is_ascii_digit()) && s.len() >= 9 && s.len() <= 12
}

pub fn is_valid_tmdb_id(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty()
}

pub fn extract_ids_from_text(text: &str) -> (Option<String>, Option<u64>) {
    let mut imdb_id: Option<String> = None;
    let mut tmdb_id: Option<u64> = None;
    
    if let Some(start) = text.find("tt") {
        let end = text[start..].find(|c: char| !c.is_ascii_digit()).unwrap_or_else(|| text[start..].len());
        let candidate = &text[start..start + end];
        if is_valid_imdb_id(candidate) {
            imdb_id = Some(candidate.to_string());
        }
    }
    
    if let Some(start) = text.find("tmdb") {
        let num_start = start + 4;
        let end = text[num_start..].find(|c: char| !c.is_ascii_digit()).unwrap_or_else(|| text[num_start..].len());
        if let Ok(id) = text[num_start..num_start + end].parse::<u64>() {
            tmdb_id = Some(id);
        }
    }
    
    if imdb_id.is_some() && tmdb_id.is_none() {
        if let Some(ref imdb) = imdb_id {
            let imdb_end = text.find(imdb).unwrap() + imdb.len();
            let remaining = &text[imdb_end..];
            if let Some(start) = remaining.find(|c: char| c.is_ascii_digit()) {
                let end = remaining[start..].find(|c: char| !c.is_ascii_digit()).unwrap_or_else(|| remaining[start..].len());
                let num_str = &remaining[start..start + end];
                if num_str.len() >= 5 && num_str.len() <= 8 {
                    if let Ok(id) = num_str.parse::<u64>() {
                        tmdb_id = Some(id);
                    }
                }
            }
        }
    }
    
    (imdb_id, tmdb_id)
}