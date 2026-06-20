use regex::Regex;

fn main() {
    let dirname = "[S04][Season 04]-[A][爱，死亡和机器人][Love, Death & Robots](2025)-tt21661768-tmdb86831";
    
    // Pattern 0a: Season folder with sort prefix + dual title + year + IMDB
    let re = Regex::new(r"^\[S\d+\]\[Season \d+\]-\[[A-Z]\]\[([^\]]+)\]\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").unwrap();
    
    if let Some(caps) = re.captures(dirname) {
        println!("Pattern matched!");
        println!("title1: {:?}", caps.get(1).map(|m| m.as_str()));
        println!("title2: {:?}", caps.get(2).map(|m| m.as_str()));
        println!("year: {:?}", caps.get(3).map(|m| m.as_str()));
        println!("imdb: {:?}", caps.get(4).map(|m| m.as_str()));
        println!("tmdb: {:?}", caps.get(5).map(|m| m.as_str()));
    } else {
        println!("Pattern NOT matched!");
    }
}
