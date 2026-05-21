
use media_organizer::generators::folder::generate_movie_folder;
use media_organizer::models::media::MovieMetadata;

fn main() {
    let metadata = MovieMetadata {
        tmdb_id: 155,
        imdb_id: Some("tt0468569".to_string()),
        original_title: "The Dark Knight".to_string(),
        title: "黑暗骑士".to_string(),
        original_language: "en".to_string(),
        year: 2008,
        ..Default::default()
    };

    let folder = generate_movie_folder(&metadata, None);
    println!("The Dark Knight (黑暗骑士):");
    println!("  Generated: {}", folder);
    println!("  Expected: [H][黑暗骑士][The Dark Knight](2008)-tt0468569-tmdb155");
    println!("  Contains [H]? {}", folder.contains("[H]"));
    println!("  Contains [D]? {}", folder.contains("[D]"));
}

