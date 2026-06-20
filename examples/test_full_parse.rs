use regex::Regex;

fn contains_chinese(s: &str) -> bool {
    s.chars().any(|c| c >= '\u{4E00}' && c <= '\u{9FFF}')
}

fn main() {
    let dirname = "[S04][Season 04]-[A][爱，死亡和机器人][Love, Death & Robots](2025)-tt21661768-tmdb86831";
    
    // Pattern 0: Season folder with dual title and year (without sort prefix)
    let re0 = Regex::new(r"^\[S\d+\]\[Season \d+\]-\[([^\]]+)\]\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").ok();
    println!("Pattern 0 compiles: {:?}", re0.is_some());
    if let Some(re) = re0 {
        println!("Pattern 0 matches: {:?}", re.captures(dirname).is_some());
    }
    
    // Pattern 0a: Season folder with sort prefix + dual title + year + IMDB
    let re0a = Regex::new(r"^\[S\d+\]\[Season \d+\]-\[[A-Z]\]\[([^\]]+)\]\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").ok();
    println!("Pattern 0a compiles: {:?}", re0a.is_some());
    if let Some(re) = re0a {
        if let Some(caps) = re.captures(dirname) {
            println!("Pattern 0a matched!");
            let title1 = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let title2 = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let title = if contains_chinese(title1) {
                title1.to_string()
            } else {
                title2.to_string()
            };
            let original_title = if contains_chinese(title2) {
                None
            } else {
                Some(title2.to_string())
            };
            let season_imdb_id = Some(format!("tt{}", caps.get(4).map(|m| m.as_str()).unwrap_or("")));
            let tmdb_id: u64 = caps.get(5).map(|m| m.as_str()).unwrap_or("0").parse().unwrap_or(0);
            println!("  title: {}", title);
            println!("  original_title: {:?}", original_title);
            println!("  season_imdb_id: {:?}", season_imdb_id);
            println!("  tmdb_id: {}", tmdb_id);
        } else {
            println!("Pattern 0a did NOT match!");
        }
    }
}
