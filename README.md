# Media Organizer

A smart media file organizer that uses AI to parse filenames and fetch metadata from TMDB, automatically renaming and organizing movie/TV show files.

## Features

- **AI-powered filename parsing** - Uses local Ollama + Qwen 2.5 model for intelligent movie/show recognition
- **IMDB ID direct lookup** - Highest priority: directly fetches metadata using IMDB ID from filename (for both movies and TV shows)
- **TMDB metadata** - Auto-fetches movie details, posters, directors, actors, and collection info
- **Smart renaming** - Renames files and folders in standardized format
- **Smart directory parsing** - Automatically skips technical subdirectories (4K/, S01/, WEB-DL/) and sorting prefixes (A_, X_, 01_)
- **Subtitle support** - Automatically moves subtitle files (.srt, .ass, .ssa, .sub, .vtt) and folders (Subs/) with video
- **Extras/Sample support** - Moves Extras, Featurettes, Sample folders and sample video files with main movie
- **Safe operations** - Generate plan first, preview, then execute with full rollback support
- **GPU acceleration** - Supports NVIDIA GPU for accelerated AI inference
- **Central indexing** - Build searchable index across multiple disks
- **Cross-disk search** - Search by title, actor, director, collection, year, genre, language
- **Export/Import** - Backup and migrate your configuration and indexes
- **Detailed logging** - Complete operation logs and progress display

## System Requirements

- **OS**: Linux (Fedora/Ubuntu/Debian)
- **Rust**: 1.70+
- **Ollama**: 0.13+ (for AI inference)
- **ffprobe**: For extracting video technical info
- **TMDB API Key**: Register at [TMDB](https://www.themoviedb.org/)

### Optional
- **NVIDIA GPU**: Recommended for accelerated AI inference (requires CUDA driver)

## Quick Start

### 1. Install Dependencies

```bash
# Fedora
sudo dnf install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Download AI model
ollama pull qwen2.5:7b
```

### 2. Configure Environment Variables

```bash
export TMDB_API_KEY="your_tmdb_api_key"
export OLLAMA_BASE_URL="http://localhost:11434"
export OLLAMA_MODEL="qwen2.5:7b"
```

### 3. Build and Run

```bash
cd media_organizer
cargo build --release

# View help
./target/release/mediaorganizer --help
```

### 4. Organize Movies

```bash
# Step 1: Generate organization plan
./target/release/mediaorganizer plan movies /path/to/movies --target /path/to/organized

# Step 2: Review the plan
cat plan_*.json

# Step 3: Execute the plan
./target/release/mediaorganizer execute plan_*.json

# Rollback if needed
./target/release/mediaorganizer rollback rollback_*.json
```

## Commands

### plan - Generate Organization Plan

```bash
mediaorganizer plan movies <SOURCE> [OPTIONS]
mediaorganizer plan tv_series <SOURCE> [OPTIONS]

Options:
  -t, --target <TARGET>  Target directory
  -v, --verbose          Verbose output
  -o, --output <OUTPUT>  Plan file output path
      --skip-preflight   Skip preflight checks
```

### execute - Execute Plan

```bash
mediaorganizer execute <PLAN_FILE> [OPTIONS]

Options:
  -o, --output <OUTPUT>  Rollback file output path
```

### rollback - Rollback Operations

```bash
mediaorganizer rollback <ROLLBACK_FILE> [OPTIONS]

Options:
  --dry-run  Dry run, show what would be done
```

### index - Build Central Index

Build a searchable index from organized media directories:

```bash
# Scan and index a directory
mediaorganizer index scan /path/to/movies --media-type movies --volume-label MyDisk1

# Scan TV shows (same disk)
mediaorganizer index scan /path/to/tv_series --media-type tv_series --volume-label MyDisk1

# Force re-scan (replace existing entries)
mediaorganizer index scan /path/to/movies --media-type movies --volume-label MyDisk1 --force

# Show statistics
mediaorganizer index stats

# List contents of a disk
mediaorganizer index list JMedia_M05

# Verify index against files
mediaorganizer index verify /path/to/movies

# Remove a disk from index
mediaorganizer index remove OldDisk --confirm

# Find duplicate media by TMDB ID across disks
mediaorganizer index duplicates

# Find only cross-volume duplicates (default)
mediaorganizer index duplicates --volume-filter cross

# Find only same-volume duplicates
mediaorganizer index duplicates --volume-filter same

# Manage movie collections
mediaorganizer index collections              # Show collection statistics
mediaorganizer index collections --update     # Update collection info from TMDB

# Manage TV series statistics
mediaorganizer index tv                      # Show TV series statistics
mediaorganizer index tv --update             # Update TV info from TMDB

# Rebuild indexes and recalculate statistics
mediaorganizer index rebuild
```

**Index Update Workflow:**
```bash
# Step 1: Scan directory (automatically rebuilds indexes)
mediaorganizer index scan /path/to/media --media-type movies --volume-label MyDisk1 --force

# Step 2 (optional): Update collection/TV info from TMDB
mediaorganizer index collections --update
mediaorganizer index tv --update
```

### search - Search Media Collection

Search across all indexed disks:

```bash
# Search by title
mediaorganizer search -t "Inception"

# Search by actor
mediaorganizer search -a "Leonardo DiCaprio"

# Search by director
mediaorganizer search -d "Christopher Nolan"

# Search by collection/series
mediaorganizer search -c "Pirates of the Caribbean"

# Search by year or year range
mediaorganizer search -y 2024
mediaorganizer search -y 2020-2024

# Search by genre
mediaorganizer search -g "Action"

# Search by language
mediaorganizer search --language zh

# Show disk online/offline status
mediaorganizer search -t "Avatar" --show-status

# Output as JSON
mediaorganizer search -t "Avatar" --format json

# Combine filters
mediaorganizer search -a "Tom Hanks" -y 2000-2020 --language en
```

### export - Export Configuration

Backup your configuration and indexes:

```bash
# Full export with auto-generated filename
mediaorganizer export --auto-name

# Export to specific file
mediaorganizer export backup.zip

# Include sensitive data (API keys)
mediaorganizer export backup.zip --include-secrets

# Only export indexes
mediaorganizer export backup.zip --only indexes

# Only export specific disk
mediaorganizer export backup.zip --disk JMedia_M05

# Add description
mediaorganizer export backup.zip --description "Pre-migration backup"

# Exclude sessions (reduce size)
mediaorganizer export backup.zip --exclude sessions
```

### import - Import Configuration

Restore configuration and indexes from backup:

```bash
# Preview what will be imported
mediaorganizer import backup.zip --dry-run

# Full import
mediaorganizer import backup.zip --force

# Merge with existing data
mediaorganizer import backup.zip --merge

# Backup existing config first
mediaorganizer import backup.zip --backup-first --force

# Only import indexes
mediaorganizer import backup.zip --only indexes
```

### sessions - Manage Sessions

```bash
mediaorganizer sessions list    # List all sessions
mediaorganizer sessions show <ID>  # Show session details
```

### verify - Verify Configuration

```bash
mediaorganizer verify <PATH>    # Verify video files
```

## Output Format

### Movie Folder Structure

Movies are organized by **original language** (from TMDB `original_language`):

```
Movies_organized/
└── ZH_Chinese/                 # Chinese language movies
    └── [Movie Name](Year)-ttIMDB_ID-tmdbTMDB_ID/
        ├── [Movie Name](Year)-WIDTHxHEIGHT(Resolution)-Format-Codec-BitDepth-Audio-Channels.mp4
        ├── movie.nfo
        ├── poster.jpg
        ├── Subs/                    (subtitle folder, if exists)
        │   └── *.srt, *.ass, ...
        ├── Extras/                  (extras folder, if exists)
        │   └── behind_the_scenes.mkv, deleted_scenes.mkv, ...
        └── Sample/                  (sample folder, if exists)
            └── preview.mp4
```

### TV Show Folder Structure

```
TV_Shows_organized/
└── EN_English/                  # English language shows
    └── [Show Name](Year)-ttIMDB_ID-tmdbTMDB_ID/
        ├── Season 01/
        │   ├── [Show Name]-S01E01-Episode Name-1920x1080(1080p)-WEB-DL.mp4
        │   └── ...
        ├── tvshow.nfo
        └── poster.jpg
```

### Language Folder Examples

| Language | Folder Name |
|----------|-------------|
| Chinese | `ZH_Chinese/` |
| English | `EN_English/` |
| Japanese | `JA_Japanese/` |
| Korean | `KO_Korean/` |
| French | `FR_French/` |
| German | `DE_German/` |
| Spanish | `ES_Spanish/` |

### Example

```
ZH_Chinese/
└── [刺杀小说家2](2025)-tt33095008-tmdb945801/
    ├── [刺杀小说家2](2025)-3840x2160(2160p)-BluRay-hevc-8bit-dts-5.1.mp4
    ├── movie.nfo
    └── poster.jpg
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `TMDB_API_KEY` | TMDB API key | (required) |
| `TMDB_BEARER_TOKEN` | TMDB Bearer token (v4) | (optional) |
| `OLLAMA_BASE_URL` | Ollama service URL | `http://localhost:11434` |
| `OLLAMA_MODEL` | AI model name | `qwen2.5:7b` |
| `RUST_LOG` | Log level | `info` |

### TMDB API Key

1. Register at [TMDB](https://www.themoviedb.org/signup)
2. Go to [API Settings](https://www.themoviedb.org/settings/api)
3. Apply for API Key (v3 auth)
4. Set environment variable: `export TMDB_API_KEY="your_key"`

## GPU Configuration

If you have an NVIDIA GPU, enable GPU acceleration for faster AI inference:

See [GPU Setup Guide](docs/en/06-gpu-setup.md)

### Quick Check

```bash
# Check GPU
nvidia-smi

# Check Ollama GPU status
ollama serve 2>&1 | grep -i "inference compute"
# Should show: library=CUDA
```

## Performance

| Mode | AI Parse Time (per file) |
|------|--------------------------|
| CPU | 30-60 seconds |
| GPU (RTX 3500) | 1-2 seconds |

## Metadata Lookup Priority

The tool uses a smart priority system for metadata lookup (works for both Movies and TV Shows):

1. **TMDB ID** (highest priority) - If path contains `tmdb12345`, fetches directly from TMDB
2. **IMDB ID** - If path contains `tt1234567`, uses TMDB's find API to lookup
3. **AI parsing + title search** - Uses Ollama AI to parse filename, then searches TMDB by title

**Path includes:** filename AND all parent directory names.

**Examples:**
```
Movie (2023) tt1234567.mkv           -> Uses IMDB ID from filename
X_许你耀眼.2025.tt32582480/01 4K.mp4  -> Uses IMDB ID from directory name
[Movie](2024)-tmdb945801/movie.mkv   -> Uses TMDB ID from directory name
```

This means files with IMDB IDs anywhere in their path can be matched even when title search fails.

## Smart Directory Parsing

The tool intelligently handles complex directory structures:

### Technical Subdirectories (auto-skipped)

When parsing directory names, the following subdirectories are automatically skipped to find the actual title:

| Category | Patterns |
|----------|----------|
| Resolution | `4K`, `1080p`, `2160p`, `720p`, `480p`, `UHD`, `HD`, `SD` |
| Season | `S01`, `S02`, `Season 1`, `Season.2`, `第1季` |
| Quality | `WEB-DL`, `BluRay`, `BDRip`, `DVDRip`, `HDTV`, `Remux` |
| Codec | `HEVC`, `x265`, `H264`, `HDR`, `HDR10`, `Dolby`, `Atmos` |

**Example:**
```
许你耀眼/4K/01 4K.mp4
         ↑
    Skipped (resolution)
Result: "许你耀眼" used as title
```

### Sorting Prefixes (auto-removed)

Common sorting prefixes are automatically removed:

| Pattern | Example | Result |
|---------|---------|--------|
| Letter + separator | `X_许你耀眼` | `许你耀眼` |
| Letter + separator | `A-剧名` | `剧名` |
| Number + separator | `01_电影名` | `电影名` |

## Subtitle Handling

When moving video files, the tool automatically detects and moves related subtitle files:

**Supported subtitle folders:**
- `Sub`, `Subs`, `Subtitle`, `Subtitles`, `字幕`

**Supported subtitle files:**
- `.srt`, `.ass`, `.ssa`, `.sub`, `.idx`, `.vtt`, `.sup`, `.smi`

Subtitles are moved **without renaming** to preserve their original naming conventions.

## Extras and Sample Handling

The tool automatically moves supplementary content with the main movie:

**Supported extras folders:**
- `Extras`, `Extra`, `Featurettes`, `Behind the Scenes`, `Deleted Scenes`, `Making of`, `Bonus`

**Supported sample folders:**
- `Sample`, `Samples`

**Sample video files:**
- Any video file with `sample` in the filename (e.g., `movie.sample.mkv`, `sample-video.avi`)

These are moved **as-is** to the organized movie folder, preserving their original structure.

## Troubleshooting

### AI Parse Timeout
- Check if Ollama is running: `pgrep ollama`
- Check if GPU is enabled: Look for `library=CUDA` in Ollama logs

### TMDB API Error
- Check if API Key is correct
- Check network connection (may need proxy in some regions)

### Video Info Extraction Failed
- Ensure ffprobe is installed: `which ffprobe`

## Documentation

### English
- [Overview](docs/en/01-overview.md)
- [Architecture](docs/en/02-architecture.md)
- [Processing Flow](docs/en/03-processing-flow.md)
- [Central Index](docs/en/04-central-index.md)
- [Export/Import](docs/en/05-export-import.md)
- [GPU Setup](docs/en/06-gpu-setup.md)

### Chinese (中文)
- [Overview (概述)](docs/zh/01-overview.md)
- [Architecture (架构设计)](docs/zh/02-architecture.md)
- [Processing Flow (处理流程)](docs/zh/03-processing-flow.md)
- [Central Index (中央索引)](docs/zh/04-central-index.md)
- [Export/Import (导入导出)](docs/zh/05-export-import.md)
- [GPU Setup (GPU配置)](docs/zh/06-gpu-setup.md)

## License

MIT License

## Contributing

Issues and Pull Requests are welcome!
