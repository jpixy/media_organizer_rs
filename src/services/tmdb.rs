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

    /// Get movie details with credits.
    pub async fn get_movie_details(&self, movie_id: u64) -> Result<MovieDetails> {
        let url = self.build_url(
            &format!("movie/{}", movie_id),
            "&append_to_response=credits,release_dates",
        );
        let resp: MovieDetails = self.request_with_retry(|| self.build_request(&url).send()).await?;
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

    /// Get episode details.
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
            "",
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
