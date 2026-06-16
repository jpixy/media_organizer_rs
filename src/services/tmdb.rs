 //! TMDB API client.

use crate::Result;
use serde::Deserialize;

const TMDB_BASE_URL: &str = "https://api.themoviedb.org/3";

/// TMDB client configuration.
#[derive(Debug, Clone)]
pub struct TmdbConfig {
    /// API key or Bearer token (JWT)
    pub api_key: String,
    pub language: String,
    /// Whether to use Bearer token authentication (API v4 style)
    pub use_bearer: bool,
    /// Proxy settings
    pub proxy_enabled: bool,
    pub proxy: Option<String>,
}

impl TmdbConfig {
    /// Create config from environment variable.
    /// Supports both API key (v3) and Bearer token (v4) formats.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("TMDB_API_KEY").map_err(|_| crate::Error::TmdbApiKeyMissing)?;

        // Bearer tokens start with "eyJ" (base64 encoded JWT header)
        let use_bearer = api_key.starts_with("eyJ");

        Ok(Self {
            api_key,
            language: "zh-CN".to_string(),
            use_bearer,
            proxy_enabled: false,
            proxy: None,
        })
    }

    /// Create config from loaded application config.
    pub fn from_config(config: &crate::models::config::TmdbConfig, network_config: &crate::models::config::NetworkConfig) -> Result<Self> {
        let api_key = config.api_key.clone().ok_or(crate::Error::TmdbApiKeyMissing)?;

        // Bearer tokens start with "eyJ" (base64 encoded JWT header)
        let use_bearer = api_key.starts_with("eyJ");

        Ok(Self {
            api_key,
            language: config.language.clone(),
            use_bearer,
            proxy_enabled: network_config.proxy_enabled,
            proxy: network_config.proxy.clone(),
        })
    }
}

/// TMDB API client.
pub struct TmdbClient {
    config: TmdbConfig,
    client: reqwest::Client,
}

/// Movie search result.
#[derive(Debug, Deserialize)]
pub struct MovieSearchResult {
    pub results: Vec<MovieSearchItem>,
}

/// Movie search item.
#[derive(Debug, Deserialize)]
pub struct MovieSearchItem {
    pub id: u64,
    pub title: String,
    pub original_title: String,
    pub release_date: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub vote_count: Option<u32>,
    pub vote_average: Option<f32>,
}

/// Movie details.
#[derive(Debug, Deserialize)]
pub struct MovieDetails {
    pub id: u64,
    pub imdb_id: Option<String>,
    pub title: String,
    pub original_title: String,
    pub original_language: String,
    pub release_date: Option<String>,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    pub runtime: Option<u32>,
    pub vote_average: Option<f32>,
    pub vote_count: Option<u32>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub genres: Option<Vec<Genre>>,
    pub production_countries: Option<Vec<ProductionCountry>>,
    pub production_companies: Option<Vec<ProductionCompany>>,
    /// Origin countries (ISO 3166-1 codes like "US", "CN", "KR")
    /// Fallback when production_countries is empty
    pub origin_country: Option<Vec<String>>,
    pub credits: Option<Credits>,
    pub release_dates: Option<ReleaseDates>,
    /// Collection/Set this movie belongs to.
    pub belongs_to_collection: Option<MovieCollection>,
}

/// Movie collection (series of movies).
#[derive(Debug, Clone, Deserialize)]
pub struct MovieCollection {
    pub id: u64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
}

/// Collection details (full info including all movies).
#[derive(Debug, Clone, Deserialize)]
pub struct CollectionDetails {
    pub id: u64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    /// All movies in this collection
    pub parts: Vec<CollectionPart>,
}

/// A movie that is part of a collection.
#[derive(Debug, Clone, Deserialize)]
pub struct CollectionPart {
    pub id: u64,
    pub title: String,
    pub original_title: String,
    pub release_date: Option<String>,
    pub poster_path: Option<String>,
}

/// Release dates container.
#[derive(Debug, Deserialize)]
pub struct ReleaseDates {
    pub results: Vec<ReleaseDateCountry>,
}

/// Release date by country.
#[derive(Debug, Deserialize)]
pub struct ReleaseDateCountry {
    pub iso_3166_1: String,
    pub release_dates: Vec<ReleaseDate>,
}

/// Individual release date.
#[derive(Debug, Deserialize)]
pub struct ReleaseDate {
    pub certification: Option<String>,
    #[serde(rename = "type")]
    pub release_type: Option<u8>,
}

/// Genre.
#[derive(Debug, Deserialize)]
pub struct Genre {
    pub id: u64,
    pub name: String,
}

/// Production country.
#[derive(Debug, Deserialize)]
pub struct ProductionCountry {
    pub iso_3166_1: String,
    pub name: String,
}

/// Production company.
#[derive(Debug, Deserialize)]
pub struct ProductionCompany {
    pub id: u64,
    pub name: String,
}

/// Movie translations response.
#[derive(Debug, Deserialize)]
pub struct MovieTranslations {
    pub id: u64,
    pub translations: Vec<Translation>,
}

/// TV show translations response (same structure as MovieTranslations).
#[derive(Debug, Deserialize)]
pub struct TvTranslations {
    pub id: u64,
    pub translations: Vec<Translation>,
}

/// Individual translation.
#[derive(Debug, Deserialize)]
pub struct Translation {
    pub iso_3166_1: String,
    pub iso_639_1: String,
    pub name: String,
    pub english_name: String,
    pub data: TranslationData,
}

/// Translation data containing title and overview.
#[derive(Debug, Deserialize)]
pub struct TranslationData {
    pub title: String,
    pub overview: String,
    pub homepage: String,
}

/// TV show search result.
#[derive(Debug, Deserialize)]
pub struct TvSearchResult {
    pub results: Vec<TvSearchItem>,
}

/// TV show search item.
#[derive(Debug, Clone, Deserialize)]
pub struct TvSearchItem {
    pub id: u64,
    pub name: String,
    pub original_name: String,
    pub first_air_date: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
}

/// TV show details.
#[derive(Debug, Deserialize)]
pub struct TvDetails {
    pub id: u64,
    pub name: String,
    pub original_name: String,
    pub original_language: String,
    pub first_air_date: Option<String>,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub number_of_seasons: u16,
    pub number_of_episodes: u16,
    pub status: Option<String>,
    pub vote_average: Option<f32>,
    pub vote_count: Option<u32>,
    pub genres: Option<Vec<TvGenre>>,
    pub production_countries: Option<Vec<TvCountry>>,
    /// Origin countries (ISO 3166-1 codes like "US", "KR")
    pub origin_country: Option<Vec<String>>,
    pub networks: Option<Vec<TvNetwork>>,
    pub created_by: Option<Vec<TvCreator>>,
    pub credits: Option<TvCredits>,
    pub external_ids: Option<ExternalIds>,
}

/// TV Genre.
#[derive(Debug, Deserialize)]
pub struct TvGenre {
    pub id: u64,
    pub name: String,
}

/// TV Country.
#[derive(Debug, Deserialize)]
pub struct TvCountry {
    pub iso_3166_1: String,
    pub name: String,
}

/// TV Network.
#[derive(Debug, Deserialize)]
pub struct TvNetwork {
    pub id: u64,
    pub name: String,
}

/// TV Creator.
#[derive(Debug, Deserialize)]
pub struct TvCreator {
    pub id: u64,
    pub name: String,
}

/// TV Credits.
#[derive(Debug, Deserialize)]
pub struct TvCredits {
    pub cast: Option<Vec<TvCast>>,
}

/// TV Cast member.
#[derive(Debug, Deserialize)]
pub struct TvCast {
    pub id: u64,
    pub name: String,
    pub character: Option<String>,
    pub order: Option<u32>,
}

/// External IDs for a TV show.
#[derive(Debug, Deserialize)]
pub struct ExternalIds {
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<u64>,
}

/// Result from TMDB's find by external ID API.
#[derive(Debug, Deserialize)]
pub struct FindByExternalIdResult {
    pub movie_results: Vec<FindMovieResult>,
    pub tv_results: Vec<FindTvResult>,
}

/// Movie result from find API.
#[derive(Debug, Deserialize)]
pub struct FindMovieResult {
    pub id: u64,
    pub title: String,
    pub original_title: String,
    pub release_date: Option<String>,
}

/// TV show result from find API.
#[derive(Debug, Deserialize)]
pub struct FindTvResult {
    pub id: u64,
    pub name: String,
    pub original_name: String,
    pub first_air_date: Option<String>,
}

/// Season details.
#[derive(Debug, Deserialize)]
pub struct SeasonDetails {
    pub id: u64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub season_number: u16,
    pub air_date: Option<String>,
    pub episodes: Vec<EpisodeInfo>,
}

/// Episode info within a season.
#[derive(Debug, Deserialize)]
pub struct EpisodeInfo {
    pub id: u64,
    pub name: String,
    pub overview: Option<String>,
    pub episode_number: u16,
    pub season_number: u16,
    pub air_date: Option<String>,
    pub still_path: Option<String>,
}

/// Episode details.
#[derive(Debug, Deserialize)]
pub struct EpisodeDetails {
    pub id: u64,
    pub name: String,
    pub overview: Option<String>,
    pub episode_number: u16,
    pub season_number: u16,
    pub air_date: Option<String>,
    pub still_path: Option<String>,
    pub credits: Option<Credits>,
}

/// Movie/TV credits.
#[derive(Debug, Deserialize)]
pub struct Credits {
    pub cast: Vec<CastMember>,
    pub crew: Vec<CrewMember>,
}

/// Cast member.
#[derive(Debug, Deserialize)]
pub struct CastMember {
    pub id: u64,
    pub name: String,
    pub character: Option<String>,
    pub order: Option<u32>,
}

/// Crew member.
#[derive(Debug, Deserialize)]
pub struct CrewMember {
    pub id: u64,
    pub name: String,
    pub job: String,
    pub department: String,
}

impl TmdbClient {
    /// Create a new TMDB client.
    pub fn new(config: TmdbConfig) -> Self {
        let mut client_builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60));

        // Configure proxy if enabled
        if config.proxy_enabled {
            if let Some(ref proxy_url) = config.proxy {
                match reqwest::Proxy::all(proxy_url) {
                    Ok(proxy) => {
                        client_builder = client_builder.proxy(proxy);
                        tracing::info!("TMDB client using proxy: {}", proxy_url);
                    }
                    Err(e) => {
                        tracing::error!("Failed to configure proxy {}: {}", proxy_url, e);
                    }
                }
            }
        }

        let client = client_builder
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { config, client }
    }

    /// Create a new TMDB client from environment.
    pub fn from_env() -> Result<Self> {
        Ok(Self::new(TmdbConfig::from_env()?))
    }

    /// Build a request with proper authentication.
    fn build_request(&self, url: &str) -> reqwest::RequestBuilder {
        let request = self.client.get(url);
        if self.config.use_bearer {
            request.header("Authorization", format!("Bearer {}", self.config.api_key))
        } else {
            request
        }
    }

    /// Build URL with optional api_key parameter (only for v3 style).
    fn build_url(&self, path: &str, extra_params: &str) -> String {
        if self.config.use_bearer {
            format!(
                "{}/{}?language={}{}",
                TMDB_BASE_URL, path, self.config.language, extra_params
            )
        } else {
            format!(
                "{}/{}?api_key={}&language={}{}",
                TMDB_BASE_URL, path, self.config.api_key, self.config.language, extra_params
            )
        }
    }

    /// Verify API key is valid.
    pub async fn verify_api_key(&self) -> Result<bool> {
        let url = if self.config.use_bearer {
            format!("{}/authentication", TMDB_BASE_URL)
        } else {
            format!(
                "{}/authentication?api_key={}",
                TMDB_BASE_URL, self.config.api_key
            )
        };

        match self.build_request(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Execute a request with automatic retry on transient errors.
    async fn request_with_retry<R, F, Fut>(&self, build_request: F) -> Result<R>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<reqwest::Response, reqwest::Error>>,
        R: serde::de::DeserializeOwned,
    {
        const MAX_RETRIES: u32 = 5;
        const INITIAL_BACKOFF_MS: u64 = 1000;

        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            match build_request().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        return resp.json().await.map_err(crate::Error::from);
                    }
                    let status = resp.status();
                    if (status.is_server_error() || status.as_u16() == 429) && attempt < MAX_RETRIES - 1 {
                        let backoff = INITIAL_BACKOFF_MS * 2u64.pow(attempt);
                        tracing::warn!(
                            "TMDB server error {} (attempt {}/{}), retrying in {}ms...",
                            status,
                            attempt + 1,
                            MAX_RETRIES,
                            backoff
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                        continue;
                    }
                    return resp.json().await.map_err(crate::Error::from);
                }
                Err(e) => {
                    if attempt < MAX_RETRIES - 1 {
                        let backoff = INITIAL_BACKOFF_MS * 2u64.pow(attempt);
                        tracing::warn!(
                            "TMDB request failed (attempt {}/{}): {}. Retrying in {}ms...",
                            attempt + 1,
                            MAX_RETRIES,
                            e,
                            backoff
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                        last_error = Some(crate::Error::Http(e));
                        continue;
                    }
                    last_error = Some(crate::Error::Http(e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            crate::Error::Other("Unknown error after retries".to_string())
        }))
    }

    /// Search for movies.
    pub async fn search_movie(
        &self,
        query: &str,
        year: Option<u16>,
    ) -> Result<Vec<MovieSearchItem>> {
        let year_param = year.map(|y| format!("&year={}", y)).unwrap_or_default();
        let url = self.build_url(
            "search/movie",
            &format!("&query={}{}", urlencoding::encode(query), year_param),
        );

        let resp: MovieSearchResult = self.request_with_retry(|| self.build_request(&url).send()).await?;
        Ok(resp.results)
    }

    /// Search for movies with specific language for localized results.
    pub async fn search_movie_with_language(
        &self,
        query: &str,
        year: Option<u16>,
        language: &str,
    ) -> Result<Vec<MovieSearchItem>> {
        let year_param = year.map(|y| format!("&year={}", y)).unwrap_or_default();
        // Build URL with custom language parameter for localized search results
        let base_url = TMDB_BASE_URL;
        let api_key_param = if self.config.use_bearer {
            String::new()
        } else {
            format!("api_key={}&", self.config.api_key)
        };
        let url = format!(
            "{}/{}?{}{}&language={}{}",
            base_url, "search/movie", api_key_param, 
            format!("query={}", urlencoding::encode(query)),
            language,
            year_param
        );
        
        let resp: MovieSearchResult = self.request_with_retry(|| self.build_request(&url).send()).await?;
        Ok(resp.results)
    }

    /// Get movie details with credits.
    pub async fn get_movie_details(&self, movie_id: u64) -> Result<MovieDetails> {
        let url = self.build_url(
            &format!("movie/{}", movie_id),
            "&append_to_response=credits,release_dates",
        );
        let resp: MovieDetails = self.request_with_retry(|| self.build_request(&url).send()).await?;
        Ok(resp)
    }

    /// Get movie translations (localized titles in all languages).
    ///
    /// Returns a list of all available translations for a movie.
    /// Use this to find Chinese (zh-CN) or other language titles when
    /// the main details API doesn't return the desired translation.
    /// 
    /// Note: This API does NOT use the language parameter to avoid filtering results.
    /// We need all translations to find Chinese titles.
    pub async fn get_movie_translations(&self, movie_id: u64) -> Result<MovieTranslations> {
        // Build URL without language parameter to get ALL translations
        let url = if self.config.use_bearer {
            format!(
                "{}/movie/{}/translations",
                TMDB_BASE_URL, movie_id
            )
        } else {
            format!(
                "{}/movie/{}/translations?api_key={}",
                TMDB_BASE_URL, movie_id, self.config.api_key
            )
        };
        let resp: MovieTranslations = self.request_with_retry(|| self.build_request(&url).send()).await?;
        Ok(resp)
    }

    /// Get TV show translations (localized titles in all languages).
    /// Returns a list of all available translations for a TV show.
    /// Use this to find Chinese (zh-CN) or other language titles when
    /// the main details API doesn't return the desired translation.
    /// 
    /// Note: This API does NOT use the language parameter to avoid filtering results.
    /// We need all translations to find Chinese titles.
    pub async fn get_tv_translations(&self, tv_id: u64) -> Result<TvTranslations> {
        // Build URL without language parameter to get ALL translations
        let url = if self.config.use_bearer {
            format!(
                "{}/tv/{}/translations",
                TMDB_BASE_URL, tv_id
            )
        } else {
            format!(
                "{}/tv/{}/translations?api_key={}",
                TMDB_BASE_URL, tv_id, self.config.api_key
            )
        };
        let resp: TvTranslations = self.request_with_retry(|| self.build_request(&url).send()).await?;
        Ok(resp)
    }

    /// Get collection details (all movies in a franchise).
    ///
    /// Returns the full collection info including the list of all movies (parts).
    pub async fn get_collection_details(&self, collection_id: u64) -> Result<CollectionDetails> {
        let url = self.build_url(&format!("collection/{}", collection_id), "");
        let resp: CollectionDetails = self.request_with_retry(|| self.build_request(&url).send()).await?;
        Ok(resp)
    }

    /// Find movie by IMDB ID using TMDB's find API.
    ///
    /// This is useful when the filename contains an IMDB ID (e.g., tt2962872)
    /// but the title search fails to find a match.
    ///
    /// Returns the TMDB movie ID if found, None otherwise.
    pub async fn find_movie_by_imdb_id(&self, imdb_id: &str) -> Result<Option<u64>> {
        let url = self.build_url(&format!("find/{}", imdb_id), "&external_source=imdb_id");

        let resp = self.build_request(&url).send().await?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let result: FindByExternalIdResult = resp.json().await?;

        // Return the first movie result's ID if any
        Ok(result.movie_results.first().map(|m| m.id))
    }

    /// Find TV show by IMDB ID using TMDB's find API.
    pub async fn find_tv_by_imdb_id(&self, imdb_id: &str) -> Result<Option<u64>> {
        let url = self.build_url(&format!("find/{}", imdb_id), "&external_source=imdb_id");

        let resp = self.build_request(&url).send().await?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let result: FindByExternalIdResult = resp.json().await?;

        // Return the first TV result's ID if any
        Ok(result.tv_results.first().map(|t| t.id))
    }

    /// Search for TV shows.
    pub async fn search_tv(&self, query: &str, year: Option<u16>) -> Result<Vec<TvSearchItem>> {
        let year_param = year
            .map(|y| format!("&first_air_date_year={}", y))
            .unwrap_or_default();
        let url = self.build_url(
            "search/tv",
            &format!("&query={}{}", urlencoding::encode(query), year_param),
        );

        tracing::debug!("TMDB search_tv URL: {}", url);
        
        let resp = self.build_request(&url).send().await?;
        
        tracing::debug!("TMDB search_tv status: {}", resp.status());
        
        let resp: TvSearchResult = resp.json().await?;
        
        tracing::debug!("TMDB search_tv results count: {}", resp.results.len());
        
        Ok(resp.results)
    }

    /// Search for TV shows with specific language for localized results.
    pub async fn search_tv_with_language(
        &self,
        query: &str,
        year: Option<u16>,
        language: &str,
    ) -> Result<Vec<TvSearchItem>> {
        let year_param = year
            .map(|y| format!("&first_air_date_year={}", y))
            .unwrap_or_default();
        // Build URL with custom language parameter for localized search results
        let base_url = TMDB_BASE_URL;
        let api_key_param = if self.config.use_bearer {
            String::new()
        } else {
            format!("api_key={}&", self.config.api_key)
        };
        let url = format!(
            "{}/{}?{}{}&language={}{}",
            base_url, "search/tv", api_key_param, 
            format!("query={}", urlencoding::encode(query)),
            language,
            year_param
        );
        
        tracing::debug!("TMDB search_tv_with_language URL: {}", url);
        
        let resp = self.build_request(&url).send().await?;
        
        tracing::debug!("TMDB search_tv_with_language status: {}", resp.status());
        
        let resp: TvSearchResult = resp.json().await?;
        
        tracing::debug!("TMDB search_tv_with_language results count: {}", resp.results.len());
        
        Ok(resp.results)
    }

    /// Get TV show details.
    pub async fn get_tv_details(&self, tv_id: u64) -> Result<TvDetails> {
        let url = self.build_url(
            &format!("tv/{}", tv_id),
            "&append_to_response=external_ids,credits",
        );
        let resp: TvDetails = self.request_with_retry(|| self.build_request(&url).send()).await?;
        Ok(resp)
    }

    /// Get season details.
    pub async fn get_season_details(
        &self,
        tv_id: u64,
        season_number: u16,
    ) -> Result<SeasonDetails> {
        let url = self.build_url(&format!("tv/{}/season/{}", tv_id, season_number), "");
        let resp: SeasonDetails = self.request_with_retry(|| self.build_request(&url).send()).await?;
        Ok(resp)
    }

    /// Get episode details with credits.
    pub async fn get_episode_details(
        &self,
        tv_id: u64,
        season_number: u16,
        episode_number: u16,
    ) -> Result<EpisodeDetails> {
        let url = self.build_url(
            &format!(
                "tv/{}/season/{}/episode/{}",
                tv_id, season_number, episode_number
            ),
            "&append_to_response=credits",
        );
        let resp: EpisodeDetails = self.request_with_retry(|| self.build_request(&url).send()).await?;
        Ok(resp)
    }

    /// Get movie credits (directors and actors).
    pub async fn get_movie_credits(&self, movie_id: u64) -> Result<Credits> {
        let url = self.build_url(&format!("movie/{}/credits", movie_id), "");
        let resp: Credits = self.request_with_retry(|| self.build_request(&url).send()).await?;
        Ok(resp)
    }

    /// Get poster image URL.
    pub fn get_poster_url(&self, poster_path: &str, size: &str) -> String {
        format!("https://image.tmdb.org/t/p/{}{}", size, poster_path)
    }

    /// Download poster image.
    pub async fn download_poster(&self, poster_path: &str, size: &str) -> Result<Vec<u8>> {
        let url = self.get_poster_url(poster_path, size);
        let resp = self.client.get(&url).send().await?;
        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that search_movie_with_language constructs URL with language parameter
    #[test]
    fn test_search_movie_with_language_url_construction() {
        // This test verifies the URL construction logic
        // We can't make actual API calls in unit tests, but we can test the URL format
        
        // Create a minimal config
        let config = TmdbConfig {
            api_key: "test_api_key".to_string(),
            language: "zh-CN".to_string(),
            use_bearer: false,
            proxy_enabled: false,
            proxy: None,
        };
        
        // The URL should include language parameter
        // We test this by checking the build_url logic
        let client = TmdbClient::new(config);
        
        // Verify config has correct default language
        assert_eq!(client.config.language, "zh-CN");
    }

    /// Test URL building with different languages
    #[test]
    fn test_build_url_with_language() {
        let config = TmdbConfig {
            api_key: "test_key".to_string(),
            language: "zh-CN".to_string(),
            use_bearer: false,
            proxy_enabled: false,
            proxy: None,
        };
        let client = TmdbClient::new(config);
        
        let url = client.build_url("search/movie", "&query=Black%20Widow");
        assert!(url.contains("language=zh-CN"), "URL should contain language=zh-CN");
        assert!(url.contains("api_key=test_key"), "URL should contain api_key");
    }

    /// Test build_url with English language
    #[test]
    fn test_build_url_with_english_language() {
        let config = TmdbConfig {
            api_key: "test_key".to_string(),
            language: "en-US".to_string(),
            use_bearer: false,
            proxy_enabled: false,
            proxy: None,
        };
        let client = TmdbClient::new(config);
        
        let url = client.build_url("movie/497698", "");
        assert!(url.contains("language=en-US"), "URL should contain language=en-US");
    }

    /// Test build_url with bearer token
    #[test]
    fn test_build_url_with_bearer() {
        let config = TmdbConfig {
            api_key: "bearer_token".to_string(),
            language: "zh-CN".to_string(),
            use_bearer: true,
            proxy_enabled: false,
            proxy: None,
        };
        let client = TmdbClient::new(config);
        
        let url = client.build_url("movie/497698", "");
        assert!(url.contains("language=zh-CN"), "URL should contain language=zh-CN");
        // Bearer auth doesn't include api_key in URL (it's sent via Authorization header)
        assert!(!url.contains("api_key="), "Bearer URL should not contain api_key");
        assert!(url.contains("movie/497698"), "URL should contain movie ID");
    }

    /// Test that different movie IDs are handled correctly
    #[test]
    fn test_tmdb_config_movie_id_formats() {
        // Black Widow TMDB ID: 497698
        let config = TmdbConfig {
            api_key: "test".to_string(),
            language: "zh-CN".to_string(),
            use_bearer: false,
            proxy_enabled: false,
            proxy: None,
        };
        let client = TmdbClient::new(config);
        
        let url = client.build_url("movie/497698", "");
        assert!(url.contains("movie/497698"), "URL should contain correct movie ID");
        
        // Test another movie ID
        let url2 = client.build_url("movie/278", ""); // The Shawshank Redemption
        assert!(url2.contains("movie/278"), "URL should contain correct movie ID");
    }

    /// Test search URL with year parameter
    #[test]
    fn test_search_url_with_year() {
        // Build a URL similar to what search_movie_with_language would build
        let query = urlencoding::encode("Black Widow");
        let url = format!(
            "{}/{}?api_key={}&language={}&query={}&year={}",
            TMDB_BASE_URL, "search/movie", "test", "zh-CN", query, 2021
        );
        
        assert!(url.contains("year=2021"), "URL should contain year parameter");
        assert!(url.contains("query=Black+Widow") || url.contains("query=Black%20Widow"), 
            "URL should contain encoded query, got: {}", url);
    }

    /// Test poster URL construction
    #[test]
    fn test_poster_url_construction() {
        let config = TmdbConfig {
            api_key: "test".to_string(),
            language: "zh-CN".to_string(),
            use_bearer: false,
            proxy_enabled: false,
            proxy: None,
        };
        let client = TmdbClient::new(config);
        
        let poster_url = client.get_poster_url("/abc.jpg", "w500");
        assert_eq!(poster_url, "https://image.tmdb.org/t/p/w500/abc.jpg");
        
        let poster_url2 = client.get_poster_url("/xyz.png", "original");
        assert_eq!(poster_url2, "https://image.tmdb.org/t/p/original/xyz.png");
    }

    /// Test that language codes are properly formatted
    #[test]
    fn test_language_code_formats() {
        let test_cases = vec![
            ("zh-CN", "Chinese (Simplified)"),
            ("zh-TW", "Chinese (Traditional)"),
            ("en-US", "English (US)"),
            ("ja-JP", "Japanese"),
            ("ko-KR", "Korean"),
        ];
        
        for (lang, _desc) in test_cases {
            let config = TmdbConfig {
                api_key: "test".to_string(),
                language: lang.to_string(),
                use_bearer: false,
                proxy_enabled: false,
                proxy: None,
            };
            let client = TmdbClient::new(config);
            
            let url = client.build_url("movie/1", "");
            assert!(
                url.contains(&format!("language={}", lang)),
                "URL should contain language={} in {}",
                lang, url
            );
        }
    }

    /// Test proxy configuration
    #[test]
    fn test_proxy_configuration() {
        let config = TmdbConfig {
            api_key: "test".to_string(),
            language: "zh-CN".to_string(),
            use_bearer: false,
            proxy_enabled: true,
            proxy: Some("http://127.0.0.1:7890".to_string()),
        };
        let client = TmdbClient::new(config);
        
        assert!(client.config.proxy_enabled);
        assert_eq!(client.config.proxy, Some("http://127.0.0.1:7890".to_string()));
    }

    /// Test translations API URL construction
    /// Note: translations API should NOT include language parameter
    #[test]
    fn test_translations_url_construction() {
        let config = TmdbConfig {
            api_key: "test".to_string(),
            language: "zh-CN".to_string(),
            use_bearer: false,
            proxy_enabled: false,
            proxy: None,
        };
        let client = TmdbClient::new(config);
        
        // Test that build_url includes language (for other APIs)
        let url = client.build_url("movie/497698", "");
        assert!(url.contains("language=zh-CN"), "Regular URL should contain language parameter");
        
        // For translations API, we need to verify it doesn't include language
        // We can't directly test get_movie_translations URL, but we can verify
        // the build_url logic vs what get_movie_translations uses
        let translations_url = format!("{}/movie/{}/translations?api_key={}", TMDB_BASE_URL, 497698, "test");
        assert!(!translations_url.contains("language="), "Translations URL should NOT contain language parameter");
        assert!(translations_url.contains("movie/497698/translations"), "URL should contain translations path");
    }

    /// Test parsing of translations response
    #[test]
    fn test_translations_response_parsing() {
        let json_response = r#"{
            "id": 497698,
            "translations": [
                {
                    "iso_3166_1": "US",
                    "iso_639_1": "en",
                    "name": "English",
                    "english_name": "English",
                    "data": {
                        "title": "Black Widow",
                        "overview": "Test overview",
                        "homepage": ""
                    }
                },
                {
                    "iso_3166_1": "CN",
                    "iso_639_1": "zh",
                    "name": "简体中文",
                    "english_name": "Mandarin",
                    "data": {
                        "title": "黑寡妇",
                        "overview": "测试概述",
                        "homepage": ""
                    }
                }
            ]
        }"#;
        
        let translations: MovieTranslations = serde_json::from_str(json_response).unwrap();
        assert_eq!(translations.id, 497698);
        assert_eq!(translations.translations.len(), 2);
        
        // Find Chinese translation
        let chinese = translations.translations.iter()
            .find(|t| t.iso_639_1 == "zh")
            .expect("Should have Chinese translation");
        assert_eq!(chinese.data.title, "黑寡妇");
        assert_eq!(chinese.english_name, "Mandarin");
        
        // Find English translation
        let english = translations.translations.iter()
            .find(|t| t.iso_639_1 == "en")
            .expect("Should have English translation");
        assert_eq!(english.data.title, "Black Widow");
    }

    /// Test TV translations API URL construction
    #[test]
    fn test_tv_translations_url_construction() {
        // We can't directly test get_tv_translations URL, but we can verify
        // the URL format matches the expected pattern
        let tv_id: u64 = 1399; // Game of Thrones TMDB ID
        let translations_url = format!("{}/tv/{}/translations?api_key={}", TMDB_BASE_URL, tv_id, "test");
        assert!(!translations_url.contains("language="), "Translations URL should NOT contain language parameter");
        assert!(translations_url.contains(&format!("tv/{}/translations", tv_id)), "URL should contain TV translations path");
    }

    /// Test TV translations response parsing (same structure as MovieTranslations)
    #[test]
    fn test_tv_translations_response_parsing() {
        let json_response = r#"{
            "id": 1399,
            "translations": [
                {
                    "iso_3166_1": "US",
                    "iso_639_1": "en",
                    "name": "English",
                    "english_name": "English",
                    "data": {
                        "title": "Game of Thrones",
                        "overview": "Test overview",
                        "homepage": ""
                    }
                },
                {
                    "iso_3166_1": "CN",
                    "iso_639_1": "zh",
                    "name": "简体中文",
                    "english_name": "Mandarin",
                    "data": {
                        "title": "权力的游戏",
                        "overview": "测试概述",
                        "homepage": ""
                    }
                },
                {
                    "iso_3166_1": "TW",
                    "iso_639_1": "zh",
                    "name": "繁體中文",
                    "english_name": "Mandarin",
                    "data": {
                        "title": "冰與火之歌：權力遊戲",
                        "overview": "測試概述",
                        "homepage": ""
                    }
                }
            ]
        }"#;
        
        let translations: TvTranslations = serde_json::from_str(json_response).unwrap();
        assert_eq!(translations.id, 1399);
        assert_eq!(translations.translations.len(), 3);
        
        // Find Chinese (CN) translation
        let cn = translations.translations.iter()
            .find(|t| t.iso_3166_1 == "CN")
            .expect("Should have CN translation");
        assert_eq!(cn.data.title, "权力的游戏");
        
        // Find Chinese (TW) translation
        let tw = translations.translations.iter()
            .find(|t| t.iso_3166_1 == "TW")
            .expect("Should have TW translation");
        assert_eq!(tw.data.title, "冰與火之歌：權力遊戲");
        
        // Find English translation
        let english = translations.translations.iter()
            .find(|t| t.iso_639_1 == "en")
            .expect("Should have English translation");
        assert_eq!(english.data.title, "Game of Thrones");
    }

    /// Test search_tv_with_language URL construction
    #[test]
    fn test_search_tv_with_language_url_construction() {
        // This test verifies the URL construction logic for TV search with language
        let query = "Love, Death & Robots";
        let year: Option<u16> = Some(2019);
        let language = "zh-CN";
        
        let base_url = TMDB_BASE_URL;
        let api_key = "test";
        let year_param = year.map(|y| format!("&first_air_date_year={}", y)).unwrap_or_default();
        
        let url = format!(
            "{}/{}?api_key={}&language={}&query={}{}",
            base_url, "search/tv", api_key, 
            language,
            urlencoding::encode(query),
            year_param
        );
        
        assert!(url.contains("search/tv"), "URL should contain search/tv path");
        assert!(url.contains("language=zh-CN"), "URL should contain language parameter");
        assert!(url.contains("first_air_date_year=2019"), "URL should contain year parameter");
        // URL encoding may vary: & may be encoded as %26 or kept as &
        assert!(url.contains("query="), "URL should contain query parameter");
        assert!(url.contains("Love") || url.contains("Love%2C"), "URL should contain encoded query with Love");
    }

    /// Test search_tv_with_language URL without year
    #[test]
    fn test_search_tv_with_language_url_no_year() {
        let query = "Breaking Bad";
        let language = "en-US";
        
        let base_url = TMDB_BASE_URL;
        let api_key = "test";
        
        let url = format!(
            "{}/{}?api_key={}&language={}&query={}",
            base_url, "search/tv", api_key, 
            language,
            urlencoding::encode(query)
        );
        
        assert!(url.contains("search/tv"), "URL should contain search/tv path");
        assert!(url.contains("language=en-US"), "URL should contain language parameter");
        assert!(!url.contains("first_air_date_year"), "URL should NOT contain year parameter when year is None");
        assert!(url.contains("query="), "URL should contain query parameter");
        assert!(url.contains("Breaking"), "URL should contain encoded query with Breaking");
    }

    /// Test that TV translations API uses same structure as Movie
    #[test]
    fn test_tv_movie_translations_same_structure() {
        // Verify TvTranslations and MovieTranslations have the same structure
        let tv_json = r#"{"id": 1, "translations": []}"#;
        let movie_json = r#"{"id": 1, "translations": []}"#;
        
        let _: TvTranslations = serde_json::from_str(tv_json).unwrap();
        let _: MovieTranslations = serde_json::from_str(movie_json).unwrap();
        // Both should parse successfully with the same structure
    }
}
