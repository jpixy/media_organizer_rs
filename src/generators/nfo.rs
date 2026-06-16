//! NFO file generator (Kodi compatible).

use crate::models::media::{EpisodeMetadata, MovieMetadata, SeasonMetadata, TvSeriesMetadata};

/// Generate movie NFO content (Kodi/Emby/Jellyfin compatible).
pub fn generate_movie_nfo(movie: &MovieMetadata) -> String {
    let mut nfo = String::new();

    nfo.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    nfo.push_str("<movie>\n");

    // Title
    nfo.push_str(&format!("  <title>{}</title>\n", escape_xml(&movie.title)));
    nfo.push_str(&format!(
        "  <originaltitle>{}</originaltitle>\n",
        escape_xml(&movie.original_title)
    ));

    // Tagline
    if let Some(ref tagline) = movie.tagline {
        if !tagline.is_empty() {
            nfo.push_str(&format!("  <tagline>{}</tagline>\n", escape_xml(tagline)));
        }
    }

    // Year and release date
    nfo.push_str(&format!("  <year>{}</year>\n", movie.year));
    if let Some(ref release_date) = movie.release_date {
        nfo.push_str(&format!("  <releasedate>{}</releasedate>\n", release_date));
        nfo.push_str(&format!("  <premiered>{}</premiered>\n", release_date));
    }

    // Runtime
    if let Some(runtime) = movie.runtime {
        nfo.push_str(&format!("  <runtime>{}</runtime>\n", runtime));
    }

    // Rating
    if let Some(rating) = movie.rating {
        nfo.push_str("  <ratings>\n");
        nfo.push_str("    <rating name=\"themoviedb\" max=\"10\" default=\"true\">\n");
        nfo.push_str(&format!("      <value>{:.1}</value>\n", rating));
        if let Some(votes) = movie.votes {
            nfo.push_str(&format!("      <votes>{}</votes>\n", votes));
        }
        nfo.push_str("    </rating>\n");
        nfo.push_str("  </ratings>\n");
    }

    // IDs
    nfo.push_str(&format!(
        "  <uniqueid type=\"tmdb\" default=\"true\">{}</uniqueid>\n",
        movie.tmdb_id
    ));
    if let Some(ref imdb_id) = movie.imdb_id {
        nfo.push_str(&format!(
            "  <uniqueid type=\"imdb\">{}</uniqueid>\n",
            imdb_id
        ));
    }

    // Plot/Overview
    if let Some(ref overview) = movie.overview {
        nfo.push_str(&format!("  <plot>{}</plot>\n", escape_xml(overview)));
        nfo.push_str(&format!("  <outline>{}</outline>\n", escape_xml(overview)));
    }

    // Certification/MPAA
    if let Some(ref cert) = movie.certification {
        nfo.push_str(&format!("  <mpaa>{}</mpaa>\n", escape_xml(cert)));
    }

    // Genres
    for genre in &movie.genres {
        nfo.push_str(&format!("  <genre>{}</genre>\n", escape_xml(genre)));
    }

    // Countries
    for country in &movie.countries {
        nfo.push_str(&format!("  <country>{}</country>\n", escape_xml(country)));
    }

    // Studios
    for studio in &movie.studios {
        nfo.push_str(&format!("  <studio>{}</studio>\n", escape_xml(studio)));
    }

    // Credits (writers/screenplay)
    for writer in &movie.writers {
        nfo.push_str(&format!("  <credits>{}</credits>\n", escape_xml(writer)));
    }

    // Directors
    for director in &movie.directors {
        nfo.push_str(&format!(
            "  <director>{}</director>\n",
            escape_xml(director)
        ));
    }

    // Actors with roles
    for (i, actor) in movie.actors.iter().enumerate() {
        nfo.push_str("  <actor>\n");
        nfo.push_str(&format!("    <name>{}</name>\n", escape_xml(actor)));
        if let Some(role) = movie.actor_roles.get(i) {
            if !role.is_empty() {
                nfo.push_str(&format!("    <role>{}</role>\n", escape_xml(role)));
            }
        }
        nfo.push_str(&format!("    <order>{}</order>\n", i));
        nfo.push_str("  </actor>\n");
    }

    // Thumb/Poster
    for poster_url in &movie.poster_urls {
        nfo.push_str(&format!(
            "  <thumb aspect=\"poster\">{}</thumb>\n",
            escape_xml(poster_url)
        ));
    }

    // Fanart/Backdrop
    if let Some(ref backdrop) = movie.backdrop_url {
        nfo.push_str("  <fanart>\n");
        nfo.push_str(&format!("    <thumb>{}</thumb>\n", escape_xml(backdrop)));
        nfo.push_str("  </fanart>\n");
    }

    // Collection/Set (for movie series like "Pirates of the Caribbean")
    if let Some(ref collection_name) = movie.collection_name {
        nfo.push_str("  <set>\n");
        nfo.push_str(&format!(
            "    <name>{}</name>\n",
            escape_xml(collection_name)
        ));
        if let Some(ref overview) = movie.collection_overview {
            nfo.push_str(&format!(
                "    <overview>{}</overview>\n",
                escape_xml(overview)
            ));
        }
        // Total movies in collection (for collection completeness tracking)
        if let Some(total) = movie.collection_total_movies {
            nfo.push_str(&format!("    <totalmovies>{}</totalmovies>\n", total));
        }
        nfo.push_str("  </set>\n");
    }

    // TMDB Collection ID (for Kodi/Emby/Jellyfin to fetch collection artwork)
    if let Some(collection_id) = movie.collection_id {
        nfo.push_str(&format!(
            "  <tmdbcollectionid>{}</tmdbcollectionid>\n",
            collection_id
        ));
    }

    nfo.push_str("</movie>\n");
    nfo
}

/// Generate TV show NFO content.
pub fn generate_tv_series_nfo(show: &TvSeriesMetadata) -> String {
    let mut nfo = String::new();

    nfo.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    nfo.push_str("<tvshow>\n");

    // Title
    nfo.push_str(&format!("  <title>{}</title>\n", escape_xml(&show.name)));
    nfo.push_str(&format!(
        "  <originaltitle>{}</originaltitle>\n",
        escape_xml(&show.original_name)
    ));

    // Year and premiere date
    nfo.push_str(&format!("  <year>{}</year>\n", show.year));
    if let Some(ref date) = show.first_air_date {
        nfo.push_str(&format!("  <premiered>{}</premiered>\n", date));
    }

    // Status and seasons/episodes
    if let Some(ref status) = show.status {
        nfo.push_str(&format!("  <status>{}</status>\n", escape_xml(status)));
    }
    nfo.push_str(&format!("  <season>{}</season>\n", show.number_of_seasons));
    nfo.push_str(&format!(
        "  <episode>{}</episode>\n",
        show.number_of_episodes
    ));

    // Ratings
    if let Some(rating) = show.rating {
        nfo.push_str("  <ratings>\n");
        nfo.push_str("    <rating name=\"tmdb\" max=\"10\" default=\"true\">\n");
        nfo.push_str(&format!("      <value>{:.1}</value>\n", rating));
        if let Some(votes) = show.votes {
            nfo.push_str(&format!("      <votes>{}</votes>\n", votes));
        }
        nfo.push_str("    </rating>\n");
        nfo.push_str("  </ratings>\n");
    }

    // IDs
    nfo.push_str(&format!(
        "  <uniqueid type=\"tmdb\" default=\"true\">{}</uniqueid>\n",
        show.tmdb_id
    ));
    if let Some(ref imdb_id) = show.imdb_id {
        nfo.push_str(&format!(
            "  <uniqueid type=\"imdb\">{}</uniqueid>\n",
            imdb_id
        ));
    }

    // Overview and tagline
    if let Some(ref overview) = show.overview {
        nfo.push_str(&format!("  <plot>{}</plot>\n", escape_xml(overview)));
    }
    if let Some(ref tagline) = show.tagline {
        if !tagline.is_empty() {
            nfo.push_str(&format!("  <tagline>{}</tagline>\n", escape_xml(tagline)));
        }
    }

    // Genres
    for genre in &show.genres {
        nfo.push_str(&format!("  <genre>{}</genre>\n", escape_xml(genre)));
    }

    // Countries
    for country in &show.countries {
        nfo.push_str(&format!("  <country>{}</country>\n", escape_xml(country)));
    }

    // Networks/Studios
    for network in &show.networks {
        nfo.push_str(&format!("  <studio>{}</studio>\n", escape_xml(network)));
    }

    // Creators
    for creator in &show.creators {
        nfo.push_str(&format!("  <credits>{}</credits>\n", escape_xml(creator)));
    }

    // Actors
    for actor in &show.actors {
        nfo.push_str("  <actor>\n");
        nfo.push_str(&format!("    <name>{}</name>\n", escape_xml(&actor.name)));
        if let Some(ref role) = actor.role {
            nfo.push_str(&format!("    <role>{}</role>\n", escape_xml(role)));
        }
        if let Some(order) = actor.order {
            nfo.push_str(&format!("    <order>{}</order>\n", order));
        }
        nfo.push_str("  </actor>\n");
    }

    // Poster
    if let Some(poster) = show.poster_urls.first() {
        nfo.push_str(&format!("  <thumb aspect=\"poster\">{}</thumb>\n", poster));
    }

    // Fanart/Backdrop
    if let Some(ref backdrop) = show.backdrop_url {
        nfo.push_str("  <fanart>\n");
        nfo.push_str(&format!("    <thumb>{}</thumb>\n", backdrop));
        nfo.push_str("  </fanart>\n");
    }

    nfo.push_str("</tvshow>\n");
    nfo
}

/// Generate episode NFO content.
pub fn generate_episode_nfo(show: &TvSeriesMetadata, episode: &EpisodeMetadata) -> String {
    let mut nfo = String::new();

    nfo.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    nfo.push_str("<episodedetails>\n");

    // Title
    nfo.push_str(&format!("  <title>{}</title>\n", escape_xml(&episode.name)));
    if let Some(ref orig_name) = episode.original_name {
        nfo.push_str(&format!(
            "  <originaltitle>{}</originaltitle>\n",
            escape_xml(orig_name)
        ));
    }

    // Show title
    nfo.push_str(&format!(
        "  <showtitle>{}</showtitle>\n",
        escape_xml(&show.name)
    ));

    // Season and episode
    nfo.push_str(&format!("  <season>{}</season>\n", episode.season_number));
    nfo.push_str(&format!(
        "  <episode>{}</episode>\n",
        episode.episode_number
    ));

    // Air date
    if let Some(ref air_date) = episode.air_date {
        nfo.push_str(&format!("  <aired>{}</aired>\n", air_date));
    }

    // Overview
    if let Some(ref overview) = episode.overview {
        nfo.push_str(&format!("  <plot>{}</plot>\n", escape_xml(overview)));
    }

    // Directors (from crew)
    for crew_member in &episode.crew {
        if crew_member.job.eq_ignore_ascii_case("Director") {
            nfo.push_str(&format!("  <director>{}</director>\n", escape_xml(&crew_member.name)));
        }
    }

    // Writers (from crew)
    for crew_member in &episode.crew {
        if crew_member.job.eq_ignore_ascii_case("Writer") || 
           crew_member.job.eq_ignore_ascii_case("Story") ||
           crew_member.job.eq_ignore_ascii_case("Teleplay") {
            nfo.push_str(&format!("  <credits>{}</credits>\n", escape_xml(&crew_member.name)));
        }
    }

    // Actors
    for actor in &episode.cast {
        nfo.push_str("  <actor>\n");
        nfo.push_str(&format!("    <name>{}</name>\n", escape_xml(&actor.name)));
        if let Some(ref role) = actor.role {
            nfo.push_str(&format!("    <role>{}</role>\n", escape_xml(role)));
        }
        nfo.push_str("  </actor>\n");
    }

    nfo.push_str("</episodedetails>\n");
    nfo
}

/// Generate season NFO content (Kodi/Emby/Jellyfin compatible).
pub fn generate_season_nfo(show: &TvSeriesMetadata, season: &SeasonMetadata) -> String {
    let mut nfo = String::new();

    nfo.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    nfo.push_str("<season>\n");

    // Season number
    nfo.push_str(&format!("  <seasonnumber>{}</seasonnumber>\n", season.season_number));

    // Title (season name)
    nfo.push_str(&format!("  <title>{}</title>\n", escape_xml(&season.name)));

    // Show title
    nfo.push_str(&format!(
        "  <showtitle>{}</showtitle>\n",
        escape_xml(&show.name)
    ));

    // Air date
    if let Some(ref air_date) = season.air_date {
        nfo.push_str(&format!("  <aired>{}</aired>\n", air_date));
    }

    // Episode count
    nfo.push_str(&format!(
        "  <episodecount>{}</episodecount>\n",
        season.episode_count
    ));

    // Overview
    if let Some(ref overview) = season.overview {
        nfo.push_str(&format!("  <plot>{}</plot>\n", escape_xml(overview)));
    }

    // Poster
    if let Some(ref poster) = season.poster_url {
        nfo.push_str(&format!("  <thumb>{}</thumb>\n", escape_xml(poster)));
    }

    // Show IDs for reference
    nfo.push_str(&format!(
        "  <uniqueid type=\"tmdb\" default=\"true\">{}</uniqueid>\n",
        show.tmdb_id
    ));
    if let Some(ref imdb_id) = show.imdb_id {
        nfo.push_str(&format!(
            "  <uniqueid type=\"imdb\">{}</uniqueid>\n",
            imdb_id
        ));
    }

    nfo.push_str("</season>\n");
    nfo
}

/// Escape special XML characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::media::{Actor, CrewMember};

    #[test]
    fn test_generate_movie_nfo() {
        let movie = MovieMetadata {
            tmdb_id: 19995,
            imdb_id: Some("tt0499549".to_string()),
            original_title: "Avatar".to_string(),
            title: "阿凡达".to_string(),
            year: 2009,
            overview: Some("一个关于潘多拉星球的故事".to_string()),
            directors: vec!["James Cameron".to_string()],
            actors: vec!["Sam Worthington".to_string(), "Zoe Saldana".to_string()],
            ..Default::default()
        };

        let nfo = generate_movie_nfo(&movie);
        assert!(nfo.contains("<title>阿凡达</title>"));
        assert!(nfo.contains("<originaltitle>Avatar</originaltitle>"));
        assert!(nfo.contains("<year>2009</year>"));
        assert!(nfo.contains("tmdb"));
        assert!(nfo.contains("tt0499549"));
    }

    #[test]
    fn test_generate_tv_series_nfo() {
        let show = TvSeriesMetadata {
            tmdb_id: 1399,
            imdb_id: Some("tt0944947".to_string()),
            name: "权力的游戏".to_string(),
            original_name: "Game of Thrones".to_string(),
            year: 2011,
            overview: Some("维斯特洛大陆的故事".to_string()),
            creators: vec!["David Benioff".to_string(), "D.B. Weiss".to_string()],
            actors: vec![
                Actor {
                    name: "Kit Harington".to_string(),
                    role: Some("Jon Snow".to_string()),
                    order: Some(0),
                },
                Actor {
                    name: "Emilia Clarke".to_string(),
                    role: Some("Daenerys Targaryen".to_string()),
                    order: Some(1),
                },
            ],
            ..Default::default()
        };

        let nfo = generate_tv_series_nfo(&show);
        assert!(nfo.contains("<title>权力的游戏</title>"));
        assert!(nfo.contains("<originaltitle>Game of Thrones</originaltitle>"));
        assert!(nfo.contains("<year>2011</year>"));
        assert!(nfo.contains("<actor>"));
        assert!(nfo.contains("<name>Kit Harington</name>"));
        assert!(nfo.contains("<role>Jon Snow</role>"));
        assert!(nfo.contains("<credits>David Benioff</credits>"));
    }

    #[test]
    fn test_generate_season_nfo() {
        let show = TvSeriesMetadata {
            tmdb_id: 1399,
            name: "权力的游戏".to_string(),
            original_name: "Game of Thrones".to_string(),
            ..Default::default()
        };

        let season = SeasonMetadata {
            season_number: 1,
            name: "Season 1".to_string(),
            overview: Some("第一季的故事".to_string()),
            air_date: Some("2011-04-17".to_string()),
            episode_count: 10,
            ..Default::default()
        };

        let nfo = generate_season_nfo(&show, &season);
        assert!(nfo.contains("<seasonnumber>1</seasonnumber>"));
        assert!(nfo.contains("<title>Season 1</title>"));
        assert!(nfo.contains("<showtitle>权力的游戏</showtitle>"));
        assert!(nfo.contains("<plot>第一季的故事</plot>"));
        assert!(nfo.contains("<aired>2011-04-17</aired>"));
        assert!(nfo.contains("<episodecount>10</episodecount>"));
    }

    #[test]
    fn test_generate_episode_nfo_with_cast_and_crew() {
        let show = TvSeriesMetadata {
            tmdb_id: 1399,
            name: "权力的游戏".to_string(),
            original_name: "Game of Thrones".to_string(),
            ..Default::default()
        };

        let episode = EpisodeMetadata {
            season_number: 1,
            episode_number: 1,
            name: "Winter Is Coming".to_string(),
            original_name: Some("Winter Is Coming".to_string()),
            air_date: Some("2011-04-17".to_string()),
            overview: Some("故事的开始".to_string()),
            cast: vec![
                Actor {
                    name: "Kit Harington".to_string(),
                    role: Some("Jon Snow".to_string()),
                    order: Some(0),
                },
                Actor {
                    name: "Sean Bean".to_string(),
                    role: Some("Ned Stark".to_string()),
                    order: Some(1),
                },
            ],
            crew: vec![
                CrewMember {
                    name: "Tim Van Patten".to_string(),
                    job: "Director".to_string(),
                    department: "Directing".to_string(),
                },
                CrewMember {
                    name: "David Benioff".to_string(),
                    job: "Writer".to_string(),
                    department: "Writing".to_string(),
                },
            ],
        };

        let nfo = generate_episode_nfo(&show, &episode);
        assert!(nfo.contains("<title>Winter Is Coming</title>"));
        assert!(nfo.contains("<showtitle>权力的游戏</showtitle>"));
        assert!(nfo.contains("<season>1</season>"));
        assert!(nfo.contains("<episode>1</episode>"));
        assert!(nfo.contains("<plot>故事的开始</plot>"));
        assert!(nfo.contains("<director>Tim Van Patten</director>"));
        assert!(nfo.contains("<credits>David Benioff</credits>"));
        assert!(nfo.contains("<actor>"));
        assert!(nfo.contains("<name>Kit Harington</name>"));
        assert!(nfo.contains("<role>Jon Snow</role>"));
        assert!(nfo.contains("<name>Sean Bean</name>"));
        assert!(nfo.contains("<role>Ned Stark</role>"));
    }

    #[test]
    fn test_generate_episode_nfo_empty_cast_crew() {
        let show = TvSeriesMetadata {
            tmdb_id: 1399,
            name: "权力的游戏".to_string(),
            original_name: "Game of Thrones".to_string(),
            ..Default::default()
        };

        let episode = EpisodeMetadata {
            season_number: 1,
            episode_number: 1,
            name: "Winter Is Coming".to_string(),
            original_name: None,
            air_date: None,
            overview: None,
            cast: Vec::new(),
            crew: Vec::new(),
        };

        let nfo = generate_episode_nfo(&show, &episode);
        assert!(nfo.contains("<title>Winter Is Coming</title>"));
        assert!(!nfo.contains("<director>"));
        assert!(!nfo.contains("<credits>"));
        assert!(!nfo.contains("<actor>"));
    }
}
