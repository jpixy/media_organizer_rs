use regex::Regex;

fn main() {
    let dirname = "[S04][Season 04]-[A][爱，死亡和机器人][Love, Death & Robots](2025)-tt21661768-tmdb86831";
    
    // Pattern 0: Season folder with dual title and year (without sort prefix)
    let re0 = Regex::new(r"^\[S\d+\]\[Season \d+\]-\[([^\]]+)\]\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$");
    println!("Pattern 0 compiles: {:?}", re0.is_ok());
    if let Ok(re) = re0 {
        println!("Pattern 0 matches: {:?}", re.captures(dirname));
    }
    
    // Pattern 0a: Season folder with sort prefix + dual title + year + IMDB
    let re0a = Regex::new(r"^\[S\d+\]\[Season \d+\]-\[[A-Z]\]\[([^\]]+)\]\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$");
    println!("Pattern 0a compiles: {:?}", re0a.is_ok());
    if let Ok(re) = re0a {
        println!("Pattern 0a matches: {:?}", re.captures(dirname).is_some());
        if let Some(caps) = re.captures(dirname) {
            println!("  title1: {:?}", caps.get(1).map(|m| m.as_str()));
            println!("  title2: {:?}", caps.get(2).map(|m| m.as_str()));
        }
    }
}
