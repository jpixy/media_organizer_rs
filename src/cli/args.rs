//! Command line argument definitions.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Media Organizer - High-performance media file organizer with AI-powered filename parsing
/// 
/// Features:
/// - AI-powered filename parsing using Ollama (optional)
/// - TMDB metadata integration
/// - Central indexing across multiple disks
/// - Full rollback support for safe operations
/// - Cross-disk search and duplicate detection
/// 
/// Quick Start:
///   media-organizer plan movies /path/to/movies -t /path/to/library
///   media-organizer execute plan_*.json
///   media-organizer index scan /path/to/library --media-type movies --volume-label MyDisk
#[derive(Parser, Debug)]
#[command(name = "media-organizer")]
#[command(author, version, about = "Organize your movie and TV show collection with AI-powered parsing")]
pub struct Cli {
    /// Enable verbose output (show detailed logs)
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Skip preflight checks (ffprobe, TMDB connection, etc.)
    #[arg(long, global = true)]
    pub skip_preflight: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate an organization plan
    Plan {
        #[command(subcommand)]
        media_type: PlanType,
    },

    /// Execute a plan file
    Execute {
        /// Path to the plan.json file
        #[arg(value_name = "PLAN_FILE")]
        plan_file: PathBuf,

        /// Output path for rollback.json
        #[arg(short, long, value_name = "OUTPUT")]
        output: Option<PathBuf>,
    },

    /// Rollback a previous execution
    Rollback {
        /// Path to the rollback.json file
        #[arg(value_name = "ROLLBACK_FILE")]
        rollback_file: PathBuf,

        /// Dry run - show what would be done
        #[arg(long)]
        dry_run: bool,
    },

    /// Manage sessions
    Sessions {
        #[command(subcommand)]
        action: SessionsAction,
    },

    /// Verify video file integrity
    Verify {
        /// Path to verify
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },

    /// Build or update the central media index
    Index {
        #[command(subcommand)]
        action: IndexAction,
    },

    /// Search the media collection
    Search {
        /// Search by title
        #[arg(short = 't', long)]
        title: Option<String>,

        /// Search by actor name
        #[arg(short = 'a', long)]
        actor: Option<String>,

        /// Search by director name
        #[arg(short = 'd', long)]
        director: Option<String>,

        /// Search by collection/series name
        #[arg(short = 'c', long)]
        collection: Option<String>,

        /// Search by year (e.g., 2024 or 2020-2024)
        #[arg(short = 'y', long)]
        year: Option<String>,

        /// Search by genre
        #[arg(short = 'g', long)]
        genre: Option<String>,

        /// Search by country code (e.g., US, CN, KR)
        #[arg(long)]
        country: Option<String>,

        /// Show disk online/offline status
        #[arg(long)]
        show_status: bool,

        /// Output format: table, simple, json
        #[arg(long, default_value = "table")]
        format: String,
    },

    /// Export configuration and indexes
    Export {
        /// Output file path (default: auto-generated with timestamp)
        #[arg(value_name = "OUTPUT")]
        output: Option<PathBuf>,

        /// Include sensitive data (API keys)
        #[arg(long)]
        include_secrets: bool,

        /// Only export specific type: indexes, config, sessions
        #[arg(long)]
        only: Option<String>,

        /// Exclude specific type: indexes, config, sessions
        #[arg(long)]
        exclude: Option<Vec<String>>,

        /// Only export specific disk's index
        #[arg(long)]
        disk: Option<String>,

        /// Description for the backup
        #[arg(long)]
        description: Option<String>,

        /// Auto-generate filename with timestamp
        #[arg(long)]
        auto_name: bool,
    },

    /// Import configuration and indexes from backup
    Import {
        /// Backup file path
        #[arg(value_name = "BACKUP_FILE")]
        backup_file: PathBuf,

        /// Dry run - preview without importing
        #[arg(long)]
        dry_run: bool,

        /// Only import specific type: indexes, config, sessions
        #[arg(long)]
        only: Option<String>,

        /// Merge with existing data (don't overwrite)
        #[arg(long)]
        merge: bool,

        /// Force overwrite without confirmation
        #[arg(long)]
        force: bool,

        /// Backup existing config before import
        #[arg(long)]
        backup_first: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum IndexAction {
    /// Scan and index a media directory
    /// 
    /// Scans the specified directory for NFO files and builds a searchable index.
    /// Automatically rebuilds indexes and recalculates statistics after scanning.
    /// 
    /// Example:
    ///   media-organizer index scan /mnt/library/movies --media-type movies --volume-label Disk_Movies_01
    ///   media-organizer index scan /mnt/library/tv --media-type tv_series --volume-label Disk_TV_01 --force
    Scan {
        /// Directory to scan for media files
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// Media type: movies or tv_series (required)
        /// 
        /// Specifies whether to scan for movies or TV series. This parameter is mandatory.
        #[arg(value_name = "TYPE")]
        media_type: String,

        /// Volume group label (auto-detected if not provided)
        /// 
        /// Labels the disk/volume for organizational purposes. Multiple directories can share
        /// the same volume label (e.g., movies and TV shows on the same physical disk).
        #[arg(long)]
        volume_label: Option<String>,

        /// Force re-index (replace existing entries for this volume)
        /// 
        /// When specified, replaces all existing entries for this volume instead of merging.
        /// Use this when the directory structure has changed significantly.
        #[arg(long)]
        force: bool,
    },

    /// Show overall collection statistics
    /// 
    /// Displays statistics for all indexed media including volume groups,
    /// movie collections, and TV series. Shows complete/incomplete counts
    /// and total sizes.
    Stats,

    /// List contents of a specific volume group
    /// 
    /// Lists all movies or TV shows in a specific volume group.
    /// 
    /// Example:
    ///   media-organizer index list Disk_Movies_01
    ///   media-organizer index list Disk_TV_01 --media-type tv_series
    List {
        /// Volume group label to list
        #[arg(value_name = "VOLUME")]
        volume_label: String,

        /// Media type filter: movies, tv_series, or all (default: all)
        #[arg(long, default_value = "all")]
        media_type: String,
    },

    /// Verify index integrity against actual files
    /// 
    /// Checks if all indexed files still exist on disk and validates NFO files.
    Verify {
        /// Path to verify
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },

    /// Remove a volume group from the central index
    /// 
    /// Removes all entries associated with the specified volume group.
    /// Use --confirm to actually delete the data.
    /// 
    /// Example:
    ///   media-organizer index remove OldDisk --confirm
    Remove {
        /// Volume group label to remove
        #[arg(value_name = "VOLUME")]
        volume_label: String,

        /// Confirm removal (required to prevent accidental deletion)
        #[arg(long)]
        confirm: bool,
    },

    /// Find duplicate movies/TV shows by TMDB ID
    /// 
    /// Identifies duplicate media files across volume groups based on TMDB ID.
    /// Useful for finding redundant copies that can be safely deleted.
    /// 
    /// Example:
    ///   media-organizer index duplicates
    ///   media-organizer index duplicates --media-type movies --volume-filter cross
    Duplicates {
        /// Media type filter: movies, tv_series, or all (default: all)
        #[arg(long, default_value = "all")]
        media_type: String,

        /// Output format: table, simple, json (default: table)
        #[arg(long, default_value = "table")]
        format: String,

        /// Volume filter: all, same, or cross (default: cross)
        /// 
        /// - all: Show all duplicates
        /// - same: Only show duplicates within the same volume group
        /// - cross: Only show duplicates across different volume groups (most useful)
        #[arg(long, default_value = "cross")]
        volume_filter: String,
    },

    /// Manage movie collections (franchise series)
    /// 
    /// Lists all movie collections and their completion status.
    /// Use --update to fetch collection information from TMDB.
    /// 
    /// Example:
    ///   media-organizer index collections
    ///   media-organizer index collections --filter complete
    ///   media-organizer index collections --update
    Collections {
        /// Filter: complete, incomplete, or all (default: all)
        #[arg(long, default_value = "all")]
        filter: String,

        /// Output format: table, simple, json (default: table)
        #[arg(long, default_value = "table")]
        format: String,

        /// Hide movie paths (show minimal info)
        #[arg(long)]
        hide_paths: bool,

        /// Update collection information from TMDB
        /// 
        /// Fetches collection details (total movies, names) from TMDB for all movies
        /// that don't have collection info. Automatically rebuilds indexes afterward.
        #[arg(long)]
        update: bool,
    },

    /// Manage TV shows with season/episode statistics
    /// 
    /// Lists all TV shows and their completion status (how many seasons/episodes owned).
    /// Use --update to fetch TV show information from TMDB.
    /// 
    /// Example:
    ///   media-organizer index tv
    ///   media-organizer index tv --filter incomplete
    ///   media-organizer index tv --update
    Tv {
        /// Filter: complete, incomplete, or all (default: all)
        #[arg(long, default_value = "all")]
        filter: String,

        /// Output format: table, simple, json (default: table)
        #[arg(long, default_value = "table")]
        format: String,

        /// Hide TV show paths (show minimal info)
        #[arg(long)]
        hide_paths: bool,

        /// Update TV show details from TMDB
        /// 
        /// Fetches TV show details (total seasons, total episodes) from TMDB.
        /// Automatically rebuilds indexes afterward.
        #[arg(long)]
        update: bool,
    },

    /// Rebuild indexes and recalculate all statistics
    /// 
    /// Recalculates collection and TV series statistics without re-scanning files.
    /// This is useful after making manual changes to NFO files.
    /// 
    /// Note: scan --force and --update commands automatically trigger this.
    /// You rarely need to run this manually.
    /// 
    /// Example:
    ///   media-organizer index rebuild
    Rebuild {
        /// Skip preflight checks
        #[arg(long)]
        skip_preflight: bool,
    },
}

#[derive(Subcommand, Debug)]
#[clap(rename_all = "snake_case")]
pub enum PlanType {
    /// Plan for movies
    Movies {
        /// Source directory containing movies
        #[arg(value_name = "SOURCE")]
        source: PathBuf,

        /// Target directory for organized movies
        #[arg(short, long, value_name = "TARGET")]
        target: Option<PathBuf>,

        /// Output path for plan.json
        #[arg(short, long, value_name = "OUTPUT")]
        output: Option<PathBuf>,
    },

    /// Plan for TV shows
    TvSeries {
        /// Source directory containing TV shows
        #[arg(value_name = "SOURCE")]
        source: PathBuf,

        /// Target directory for organized TV shows
        #[arg(short, long, value_name = "TARGET")]
        target: Option<PathBuf>,

        /// Output path for plan.json
        #[arg(short, long, value_name = "OUTPUT")]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum SessionsAction {
    /// List all sessions
    List,

    /// Show details of a specific session
    Show {
        /// Session ID
        #[arg(value_name = "SESSION_ID")]
        session_id: String,
    },
}
