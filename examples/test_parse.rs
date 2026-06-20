fn main() {
    let dirname = "[S04][Season 04]-[A][爱，死亡和机器人][Love, Death & Robots](2025)-tt21661768-tmdb86831";
    
    // Simulate the parser logic
    let result = parse_test(dirname);
    println!("Result: {:?}", result);
}

fn parse_test(dirname: &str) -> Option<(String, u64, Option<String>)> {
    println!("Parsing: {}", dirname);
    
    // Pattern 0
    let re0 = regex::Regex::new(r"^\[S\d+\]\[Season \d+\]-\[([^\]]+)\]\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").ok()?;
    println!("Pattern 0 compiled");
    
    if let Some(caps) = re0.captures(dirname) {
        println!("Pattern 0 matched");
        return Some((caps.get(1)?.as_str().to_string(), caps.get(5)?.as_str().parse().ok()?, None));
    }
    println!("Pattern 0 did not match");
    
    // Pattern 0a
    let re0a = regex::Regex::new(r"^\[S\d+\]\[Season \d+\]-\[[A-Z]\]\[([^\]]+)\]\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").ok()?;
    println!("Pattern 0a compiled");
    
    if let Some(caps) = re0a.captures(dirname) {
        println!("Pattern 0a matched");
        let season_imdb = Some(format!("tt{}", caps.get(4)?.as_str()));
        return Some((caps.get(1)?.as_str().to_string(), caps.get(5)?.as_str().parse().ok()?, season_imdb));
    }
    println!("Pattern 0a did not match");
    
    None
}
