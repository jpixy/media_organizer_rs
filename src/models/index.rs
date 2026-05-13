//! Central index data structures for cross-disk media management.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Central index containing all media information across all disks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CentralIndex {
    /// Schema version
    pub version: String,
    /// Creation timestamp
    pub created_at: String,
    /// Last update timestamp
    pub updated_at: String,
    /// Indexed disks
    pub disks: HashMap<String, DiskInfo>,
    /// All indexed movies
    pub movies: Vec<MovieEntry>,
    /// All indexed TV shows
    pub tv_series: Vec<TvSeriesEntry>,
    /// Movie collections (series like Pirates of the Caribbean)
    pub collections: HashMap<u64, CollectionInfo>,
    /// Search indexes for fast lookup
    pub indexes: SearchIndexes,
    /// Collection statistics
    pub statistics: IndexStatistics,
}

impl Default for CentralIndex {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            disks: HashMap::new(),
            movies: Vec::new(),
            tv_series: Vec::new(),
            collections: HashMap::new(),
            indexes: SearchIndexes::default(),
            statistics: IndexStatistics::default(),
        }
    }
}

/// Information about an indexed disk.
///
/// Supports composite storage: one disk label can have multiple media types
/// (movies and tv_series) with different paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    /// Disk label (user-friendly name)
    pub label: String,
    /// Disk UUID (hardware identifier)
    pub uuid: Option<String>,
    /// Last indexing timestamp
    pub last_indexed: String,
    /// Number of movies on this disk
    pub movie_count: usize,
    /// Number of TV shows on this disk
    pub tv_series_count: usize,
    /// Total size in bytes
    pub total_size_bytes: u64,
    /// Base path when indexed (legacy, for backward compatibility)
    #[serde(default)]
    pub base_path: String,
    /// Paths by media type: {"movies": "/path/Movies", "tv_series": "/path/TV_Series"}
    /// Extensible for future media types (e.g., "music", "audiobooks")
    #[serde(default)]
    pub paths: HashMap<String, String>,
    /// Content hash for idempotency checks
    #[serde(default)]
    pub content_hash: String,
}

/// A movie entry in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieEntry {
    /// Unique identifier
    pub id: String,
    /// Disk label where this movie is stored
    pub disk: String,
    /// Disk UUID
    pub disk_uuid: Option<String>,
    /// Relative path from disk root
    pub relative_path: String,
    /// Movie title (localized)
    pub title: String,
    /// Original title
    pub original_title: Option<String>,
    /// Release year
    pub year: Option<u16>,
    /// TMDB ID
    pub tmdb_id: Option<u64>,
    /// IMDB ID
    pub imdb_id: Option<String>,
    /// Collection ID (for movie series)
    pub collection_id: Option<u64>,
    /// Collection name
    pub collection_name: Option<String>,
    /// Total movies in the collection (for completeness tracking)
    pub collection_total_movies: Option<usize>,
    /// Country code (e.g., "US", "CN")
    pub country: Option<String>,
    /// Genres
    pub genres: Vec<String>,
    /// Actors
    pub actors: Vec<String>,
    /// Directors
    pub directors: Vec<String>,
    /// Runtime in minutes
    pub runtime: Option<u32>,
    /// Rating (0-10)
    pub rating: Option<f32>,
    /// File size in bytes
    pub size_bytes: u64,
    /// Video resolution (e.g., "1080p", "4K")
    pub resolution: Option<String>,
    /// When this entry was indexed
    pub indexed_at: String,
}

/// A TV show entry in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvSeriesEntry {
    /// Unique identifier
    pub id: String,
    /// Disk label where this TV show is stored
    pub disk: String,
    /// Disk UUID
    pub disk_uuid: Option<String>,
    /// Relative path from disk root
    pub relative_path: String,
    /// TV show title (localized)
    pub title: String,
    /// Original title
    pub original_title: Option<String>,
    /// First air year
    pub year: Option<u16>,
    /// TMDB ID
    pub tmdb_id: Option<u64>,
    /// IMDB ID
    pub imdb_id: Option<String>,
    /// Country code
    pub country: Option<String>,
    /// Genres
    pub genres: Vec<String>,
    /// Actors
    pub actors: Vec<String>,
    /// Total number of seasons (from TMDB)
    pub seasons: u16,
    /// Total number of episodes (from TMDB)
    pub episodes: u32,
    /// Number of seasons owned
    #[serde(default)]
    pub owned_seasons: u16,
    /// Number of episodes owned
    #[serde(default)]
    pub owned_episodes: u32,
    /// Total size in bytes
    pub size_bytes: u64,
    /// When this entry was indexed
    pub indexed_at: String,
}

/// Movie collection information (e.g., "Pirates of the Caribbean Collection").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    /// TMDB collection ID
    pub id: u64,
    /// Collection name
    pub name: String,
    /// Poster URL
    pub poster_url: Option<String>,
    /// Movies in this collection
    pub movies: Vec<CollectionMovie>,
    /// Total movies in this collection (from TMDB)
    pub total_in_collection: usize,
    /// Number of movies owned
    pub owned_count: usize,
}

/// A movie within a collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionMovie {
    /// TMDB ID
    pub tmdb_id: u64,
    /// Movie title
    pub title: String,
    /// Release year
    pub year: Option<u16>,
    /// Disk label (None if not owned)
    pub disk: Option<String>,
    /// Whether this movie is owned
    pub owned: bool,
}

/// Search indexes for fast lookup.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchIndexes {
    /// Index by actor name -> list of movie/tvshow IDs
    pub by_actor: HashMap<String, Vec<String>>,
    /// Index by director name -> list of movie IDs
    pub by_director: HashMap<String, Vec<String>>,
    /// Index by genre -> list of movie/tvshow IDs
    pub by_genre: HashMap<String, Vec<String>>,
    /// Index by year -> list of movie/tvshow IDs
    pub by_year: HashMap<u16, Vec<String>>,
    /// Index by country code -> list of movie/tvshow IDs
    pub by_country: HashMap<String, Vec<String>>,
    /// Index by collection ID -> list of movie IDs
    pub by_collection: HashMap<u64, Vec<String>>,
}

/// Collection statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStatistics {
    /// Total number of movies
    pub total_movies: usize,
    /// Total number of TV shows
    pub total_tv_series: usize,
    /// Total number of disks
    pub total_disks: usize,
    /// Total size in bytes
    pub total_size_bytes: u64,
    /// Complete collections count
    #[serde(default)]
    pub complete_collections: usize,
    /// Incomplete collections count
    #[serde(default)]
    pub incomplete_collections: usize,
    /// TV shows with all seasons/episodes owned
    #[serde(default)]
    pub complete_tv_series: usize,
    /// TV shows with partial seasons/episodes owned
    #[serde(default)]
    pub incomplete_tv_series: usize,
}

/// Individual disk index (stored separately for each disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIndex {
    /// Schema version
    pub version: String,
    /// Disk information
    pub disk: DiskInfo,
    /// Movies on this disk
    pub movies: Vec<MovieEntry>,
    /// TV shows on this disk
    pub tv_series: Vec<TvSeriesEntry>,
}

impl Default for DiskIndex {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            disk: DiskInfo {
                label: String::new(),
                uuid: None,
                last_indexed: chrono::Utc::now().to_rfc3339(),
                movie_count: 0,
                tv_series_count: 0,
                total_size_bytes: 0,
                base_path: String::new(),
                paths: HashMap::new(),
                content_hash: String::new(),
            },
            movies: Vec::new(),
            tv_series: Vec::new(),
        }
    }
}

/// Export manifest for backup files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    /// Schema version
    pub version: String,
    /// Application version
    pub app_version: String,
    /// Creation timestamp
    pub created_at: String,
    /// Creator (user@hostname)
    pub created_by: String,
    /// User-provided description
    pub description: Option<String>,
    /// Export contents summary
    pub contents: ExportContents,
    /// Statistics
    pub statistics: ExportStatistics,
    /// Source path information
    pub source_paths: SourcePaths,
}

/// What's included in the export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportContents {
    /// Whether config is included
    pub config: bool,
    /// Whether central index is included
    pub central_index: bool,
    /// List of disk indexes included
    pub disk_indexes: Vec<String>,
    /// Number of sessions included
    pub sessions: usize,
    /// Whether secrets (API keys) are included
    pub includes_secrets: bool,
}

/// Export statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportStatistics {
    pub total_movies: usize,
    pub total_tv_series: usize,
    pub total_disks: usize,
    pub total_sessions: usize,
    pub export_size_bytes: u64,
}

/// Source path information for the export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePaths {
    pub config_dir: String,
    pub hostname: String,
}

impl CentralIndex {
    /// Rebuild search indexes from movie and tvshow entries.
    pub fn rebuild_indexes(&mut self) {
        self.indexes = SearchIndexes::default();

        // Index movies
        for movie in &self.movies {
            // By actor
            for actor in &movie.actors {
                self.indexes
                    .by_actor
                    .entry(actor.clone())
                    .or_default()
                    .push(movie.id.clone());
            }

            // By director
            for director in &movie.directors {
                self.indexes
                    .by_director
                    .entry(director.clone())
                    .or_default()
                    .push(movie.id.clone());
            }

            // By genre
            for genre in &movie.genres {
                self.indexes
                    .by_genre
                    .entry(genre.clone())
                    .or_default()
                    .push(movie.id.clone());
            }

            // By year
            if let Some(year) = movie.year {
                self.indexes
                    .by_year
                    .entry(year)
                    .or_default()
                    .push(movie.id.clone());
            }

            // By country
            if let Some(ref country) = movie.country {
                self.indexes
                    .by_country
                    .entry(country.clone())
                    .or_default()
                    .push(movie.id.clone());
            }

            // By collection
            if let Some(collection_id) = movie.collection_id {
                self.indexes
                    .by_collection
                    .entry(collection_id)
                    .or_default()
                    .push(movie.id.clone());

                // Build collection info
                let collection = self.collections.entry(collection_id).or_insert_with(|| {
                    CollectionInfo {
                        id: collection_id,
                        name: movie
                            .collection_name
                            .clone()
                            .unwrap_or_else(|| "Unknown Collection".to_string()),
                        poster_url: None,
                        movies: Vec::new(),
                        total_in_collection: 0, // Will be updated from NFO if available
                        owned_count: 0,
                    }
                });

                // Update total_in_collection from NFO data if available and not already set
                if let Some(total) = movie.collection_total_movies {
                    // Use the maximum value seen (in case different NFOs have different info)
                    if total > collection.total_in_collection {
                        collection.total_in_collection = total;
                    }
                }

                // Add movie to collection if not already present
                let already_in_collection = collection
                    .movies
                    .iter()
                    .any(|m| m.tmdb_id == movie.tmdb_id.unwrap_or(0));
                if !already_in_collection {
                    collection.movies.push(CollectionMovie {
                        tmdb_id: movie.tmdb_id.unwrap_or(0),
                        title: movie.title.clone(),
                        year: movie.year,
                        disk: Some(movie.disk.clone()),
                        owned: true,
                    });
                    collection.owned_count += 1;
                }
            }
        }

        // Index TV shows
        for tvshow in &self.tv_series {
            // By actor
            for actor in &tvshow.actors {
                self.indexes
                    .by_actor
                    .entry(actor.clone())
                    .or_default()
                    .push(tvshow.id.clone());
            }

            // By genre
            for genre in &tvshow.genres {
                self.indexes
                    .by_genre
                    .entry(genre.clone())
                    .or_default()
                    .push(tvshow.id.clone());
            }

            // By year
            if let Some(year) = tvshow.year {
                self.indexes
                    .by_year
                    .entry(year)
                    .or_default()
                    .push(tvshow.id.clone());
            }

            // By country
            if let Some(ref country) = tvshow.country {
                self.indexes
                    .by_country
                    .entry(country.clone())
                    .or_default()
                    .push(tvshow.id.clone());
            }
        }
    }

    /// Update statistics from current data.
    pub fn update_statistics(&mut self) {
        self.statistics.total_movies = self.movies.len();
        self.statistics.total_tv_series = self.tv_series.len();
        self.statistics.total_disks = self.disks.len();
        self.statistics.total_size_bytes = self.movies.iter().map(|m| m.size_bytes).sum::<u64>()
            + self.tv_series.iter().map(|t| t.size_bytes).sum::<u64>();

        // Collections
        // Use total_in_collection from TMDB if available, otherwise use heuristics.
        self.statistics.complete_collections = self
            .collections
            .values()
            .filter(|c| {
                if c.total_in_collection > 0 {
                    // If we know the total, check if we have all movies
                    c.owned_count >= c.total_in_collection
                } else {
                    // Fallback heuristic: 2+ movies means likely complete
                    c.owned_count >= 2
                }
            })
            .count();
        self.statistics.incomplete_collections = self
            .collections
            .values()
            .filter(|c| {
                if c.total_in_collection > 0 {
                    // If we know the total, check if we're missing some
                    c.owned_count > 0 && c.owned_count < c.total_in_collection
                } else {
                    // Fallback: single movie = likely incomplete
                    c.owned_count == 1
                }
            })
            .count();

        // TV Series completeness
        // Check if owned seasons/episodes match total seasons/episodes
        self.statistics.complete_tv_series = self
            .tv_series
            .iter()
            .filter(|t| {
                t.seasons > 0 && t.owned_seasons > 0 && t.owned_seasons >= t.seasons
            })
            .count();
        self.statistics.incomplete_tv_series = self
            .tv_series
            .iter()
            .filter(|t| {
                t.seasons > 0 && t.owned_seasons > 0 && t.owned_seasons < t.seasons
            })
            .count();
    }

    /// Merge another index into this one (for import --merge).
    pub fn merge(&mut self, other: CentralIndex) {
        // Merge disks
        for (label, disk) in other.disks {
            self.disks.entry(label).or_insert(disk);
        }

        // Merge movies (avoid duplicates by tmdb_id)
        let existing_tmdb_ids: std::collections::HashSet<_> =
            self.movies.iter().filter_map(|m| m.tmdb_id).collect();

        for movie in other.movies {
            if let Some(tmdb_id) = movie.tmdb_id {
                if !existing_tmdb_ids.contains(&tmdb_id) {
                    self.movies.push(movie);
                }
            } else {
                self.movies.push(movie);
            }
        }

        // Merge TV shows
        let existing_tv_series_ids: std::collections::HashSet<_> =
            self.tv_series.iter().filter_map(|t| t.tmdb_id).collect();

        for tvshow in other.tv_series {
            if let Some(tmdb_id) = tvshow.tmdb_id {
                if !existing_tv_series_ids.contains(&tmdb_id) {
                    self.tv_series.push(tvshow);
                }
            } else {
                self.tv_series.push(tvshow);
            }
        }

        // Merge collections
        for (id, collection) in other.collections {
            self.collections.entry(id).or_insert(collection);
        }

        // Rebuild indexes and statistics
        self.rebuild_indexes();
        self.update_statistics();
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}
