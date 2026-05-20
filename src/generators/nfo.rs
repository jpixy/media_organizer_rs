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
}
