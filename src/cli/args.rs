//! Command line argument definitions.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Media Organizer - Organize your video files with AI
#[derive(Parser, Debug)]
#[command(name = "media-organizer")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Skip preflight checks
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
    /// Scan and index a directory
    Scan {
        /// Directory to scan
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// Media type: movies or tv_series
        #[arg(value_name = "TYPE")]
        media_type: String,

        /// Volume group label (auto-detected if not provided)
        #[arg(long)]
        volume_label: Option<String>,

        /// Force re-index (replace existing entries)
        #[arg(long)]
        force: bool,
    },

    /// Show collection statistics
    Stats,

    /// List contents of a specific volume group
    List {
        /// Volume group label to list
        #[arg(value_name = "VOLUME")]
        volume_label: String,

        /// Media type filter: movies, tv_series, or all
        #[arg(long, default_value = "all")]
        media_type: String,
    },

    /// Verify index against actual files
    Verify {
        /// Path to verify
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },

    /// Remove a volume group from the index
    Remove {
        /// Volume group label to remove
        #[arg(value_name = "VOLUME")]
        volume_label: String,

        /// Confirm removal
        #[arg(long)]
        confirm: bool,
    },

    /// Find duplicate movies/TV shows by TMDB ID across disks
    Duplicates {
        /// Media type filter: movies, tv_series, or all
        #[arg(long, default_value = "all")]
        media_type: String,

        /// Output format: table, simple, json
        #[arg(long, default_value = "table")]
        format: String,
    },

    /// List movie collections (franchise series)
    Collections {
        /// Filter: complete, incomplete, or all
        #[arg(long, default_value = "all")]
        filter: String,

        /// Output format: table, simple, json
        #[arg(long, default_value = "table")]
        format: String,

        /// Hide movie paths (show minimal info)
        #[arg(long)]
        hide_paths: bool,

        /// Update collection totals from TMDB and write back to NFO files
        #[arg(long)]
        update: bool,
    },

    /// List TV shows with season/episode statistics
    Tv {
        /// Filter: complete, incomplete, or all
        #[arg(long, default_value = "all")]
        filter: String,

        /// Output format: table, simple, json
        #[arg(long, default_value = "table")]
        format: String,

        /// Hide TV show paths (show minimal info)
        #[arg(long)]
        hide_paths: bool,

        /// Update TV show details from TMDB and write back to NFO files
        #[arg(long)]
        update: bool,
    },

    /// Rebuild indexes and recalculate all statistics
    Rebuild {
        /// Skip preflight checks
        #[arg(long)]
        skip_preflight: bool,
    },
}

#[derive(Subcommand, Debug)]
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
