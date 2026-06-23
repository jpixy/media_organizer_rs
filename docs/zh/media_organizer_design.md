# Media Organizer 技术文档

## 1. 简介与适用场景

本文档详细描述 Media Organizer 媒体文件整理工具的核心逻辑，包括中文译名获取、TMDB API 演员信息获取、文件夹和文件命名规范等。

### 1.1 支持的媒体类型
- **电影 (Movies)**: 单个视频文件或有多章节的影片
- **电视剧 (TV Series)**: 包含多季、多集的电视节目

### 1.2 核心功能
- 自动识别视频文件并匹配 TMDB 元数据
- 生成规范化的文件夹和文件名
- 支持中文译名自动获取
- 自动下载海报和 NFO 文件

---

## 2. 中文译名获取逻辑

### 2.1 优先级策略

系统使用 **TMDB Translations API** 获取中文译名，遵循以下优先级：

```
CN (中国大陆) > SG (新加坡) > HK (香港) > TW (台湾) > 其他中文翻译
```

### 2.2 源码实现

核心函数 `find_priority_chinese_title` 定义在 `src/core/planner.rs`:

```rust
pub fn find_priority_chinese_title(candidates: &[(String, String)]) -> Option<String> {
    let region_priority = ["CN", "SG", "HK", "TW"];
    
    // First pass: try in priority order
    for priority_region in &region_priority {
        if let Some((_region, title)) = candidates.iter().find(|(r, _)| r == priority_region) {
            return Some(title.clone());
        }
    }
    
    // Final fallback: use any available Chinese translation
    candidates.first().map(|(_, title)| title.clone())
}
```

### 2.3 中文译名获取流程

```
开始
  │
  ▼
从 TMDB 获取翻译列表
  │
  ▼
检查 CN 翻译 ─── 存在 ───► 使用 CN 翻译
  │
  │ 不存在
  ▼
检查 SG 翻译 ─── 存在 ───► 使用 SG 翻译
  │
  │ 不存在
  ▼
检查 HK 翻译 ─── 存在 ───► 使用 HK 翻译
  │
  │ 不存在
  ▼
检查 TW 翻译 ─── 存在 ───► 使用 TW 翻译
  │
  │ 不存在
  ▼
使用任意中文翻译
  │
  ▼
返回中文译名
```

### 2.4 TMDB Translations API 响应格式

```json
{
  "id": 86831,
  "translations": [
    {
      "iso_3166_1": "CN",
      "iso_639_1": "zh",
      "name": "简体中文",
      "english_name": "Mandarin",
      "data": {
        "name": "爱，死亡和机器人",
        "overview": "...",
        "homepage": "",
        "tagline": ""
      }
    },
    {
      "iso_3166_1": "TW",
      "iso_639_1": "zh",
      "name": "繁體中文",
      "english_name": "Mandarin",
      "data": {
        "name": "愛 x 死 x 機器人",
        "overview": "...",
        "homepage": "",
        "tagline": ""
      }
    }
  ]
}
```

### 2.5 目录名作为备用源

当 TMDB API 无法返回有效中文译名时，系统会尝试从 **目录名** 中提取中文标题：

```rust
// 目录名示例: "爱，死亡和机器人 第四季 Love, Death & Robots Season 4 (2025)"
let is_likely_tvshow_folder = name.contains("Season") || ...;
if is_likely_tvshow_folder {
    // 提取中文标题
    if let Some(title) = parser::extract_title_from_dirname(name) {
        // 使用提取的中文标题
    }
}
```

---

## 3. TMDB API 演员信息获取

### 3.1 API 端点

| 媒体类型 | API 端点 | 说明 |
|---------|---------|------|
| 电影 | `/movie/{movie_id}/credits` | 获取电影演员和剧组信息 |
| 电视剧 | `/tv/{tv_id}/season/{season_number}/credits` | 获取季演员信息 |
| 电视剧 | `/tv/{tv_id}/credits` | 获取剧集演员信息 |

### 3.2 Credits 数据结构

```rust
pub struct Credits {
    pub id: u64,
    pub cast: Vec<CastMember>,      // 演员列表
    pub crew: Vec<CrewMember>,       // 剧组人员
}

pub struct CastMember {
    pub id: u64,
    pub name: String,               // 演员姓名
    pub character: String,           // 饰演角色
    pub order: u8,                  // 排序顺序
    pub profile_path: Option<String>,
}

pub struct CrewMember {
    pub id: u64,
    pub name: String,
    pub job: String,                // 职位 (Director, Producer 等)
    pub department: String,         // 部门
}
```

### 3.3 演员信息使用场景

1. **NFO 文件生成**: 在 NFO 文件中包含演员和导演信息
2. **海报下载**: 目前仅支持主海报，不下载演员图

### 3.4 源码实现

获取电影演员:
```rust
pub async fn get_movie_credits(&self, movie_id: u64) -> Result<Credits> {
    let url = format!("{}/movie/{}/credits", self.api_key, movie_id);
    let response = self.client.get(&url).send().await?;
    let credits: Credits = response.json().await?;
    Ok(credits)
}
```

---

## 4. 文件夹命名规范

### 4.1 电影文件夹格式

```
[{排序前缀}][{中文标题}][{原始英文标题}]({年份})-{imdb_id}-tmdb{tmdb_id}
```

**示例**:
```
[N][女人的碎片][Pieces of a Woman](2020)-tt11161474-tmdb641662
[T][特工迷阵][The Wrecking Crew](2026)-tt33046197-tmdb1168190
```

### 4.2 电视剧文件夹格式

```
[{排序前缀}][{中文标题}][{原始英文标题}]({年份})-{imdb_id}-tmdb{tmdb_id}
```

**示例**:
```
[A][爱，死亡和机器人][Love, Death & Robots](2025)-tt9561862-tmdb450504
[Z][终极名单][The Terminal List](2022)-tt1862754-tmdb186250
```

### 4.3 季文件夹格式

```
[S{季号}][Season {季号}]-[{排序前缀}][{中文标题}][{原始英文标题}]-{imdb_id}-tmdb{tmdb_id}
```

**示例**:
```
[S04][Season 04]-[A][爱，死亡和机器人][Love, Death & Robots]-tt9561862-tmdb450504
[S01][Season 01]-[Z][终极名单][The Terminal List]-tt1862754-tmdb186250
```

### 4.4 TV 组织结构详解

电视剧采用**三层嵌套结构**：TV Show → Season → Episode。每层有不同的元数据和命名规则。

#### 4.4.1 TV Show Level（电视剧级别）

**定义**: 代表整个电视剧系列，包含所有季的共同信息。

**包含的元数据**:

| 字段 | 说明 | 示例 |
|------|------|------|
| `name` | 中文/本地化标题 | 爱，死亡和机器人 |
| `original_name` | 原始英文标题 | Love, Death & Robots |
| `year` | 首播年份 | 2025 |
| `imdb_id` | IMDB 剧集 ID | tt9561862 |
| `tmdb_id` | TMDB 剧集 ID | 450504 |
| `original_language` | 原始语言代码 | en |

**文件夹命名**:
```
[{排序前缀}][{中文标题}][{原始英文标题}]({年份})-{imdb_id}-tmdb{tmdb_id}
```

#### 4.4.2 Season Level（季级别）

**定义**: 代表电视剧的一个特定季度。

**包含的元数据**:

| 字段 | 说明 | 示例 |
|------|------|------|
| `season_number` | 季号 | 4 |
| `name` | 季名称（可选） | Season 4 |
| `air_date` | 首播日期 | 2025-05-15 |
| `tmdb_id` | TMDB Season ID（可能为0） | 142356 |

**关键特性**:
- **IMDB**: 没有独立的 Season ID，使用 Show 级别的 IMDB ID
- **TMDB**: 每季有独立的 TMDB Season ID，但可能返回 0
- **备用机制**: 当 TMDB Season ID 为 0 时，使用 Show 级别的 TMDB ID

**文件夹命名**:
```
[S{季号}][Season {季号}]-[{排序前缀}][{中文标题}][{原始英文标题}]-{imdb_id}-tmdb{tmdb_id}
```

#### 4.4.3 Episode Level（集级别）

**定义**: 代表一季中的某一集。

**包含的元数据**:

| 字段 | 说明 | 示例 |
|------|------|------|
| `season_number` | 所属季号 | 4 |
| `episode_number` | 集号 | 1 |
| `name` | 集标题（中文） | 停不下来！ |
| `original_name` | 集标题（英文） | Stop Motion |
| `air_date` | 播出日期 | 2025-05-15 |

**文件命名**:
```
[{集号}][-{集名}][-{排序前缀}][{标题}][{原始标题}]-{分辨率}({清晰度})-{编码}-{比特深度}.{扩展名}
```

#### 4.4.4 完整目录结构示例

```
TV_00_TMP/
└── EN_English/                                    # 语言文件夹
    └── [A][爱，死亡和机器人][Love, Death & Robots](2025)-tt9561862-tmdb450504/  # TV Show 文件夹
        ├── poster.jpg                              # 海报
        ├── tvshow.nfo                              # Show 级 NFO
        └── [S04][Season 04]-[A][爱，死亡和机器人][Love, Death & Robots]-tt9561862-tmdb450504/  # Season 文件夹
            ├── season.nfo                          # Season 级 NFO
            ├── S04E01-[停不下来！]-[L][Love, Death & Robots]-1920x1080(1080p)-h264-8bit-eac3-5.1.mp4  # Episode 文件
            ├── S04E02-[...]...mp4
            └── ...
```

#### 4.4.5 各 Level 的 ID 使用规则

```
TV Show Level ──────────────────────────────► Season Level ──────────────────────────────► Episode Level

     │                                         │                                           │
     ├── imdb_id: tt9561862 (固定)            ├── imdb_id: 继承自 Show 或使用独立 Season ID ──► imdb_id: 继承自 Season/Show
     │                                         │   (单元剧专用)                              │
     └── tmdb_id: 450504                       └── tmdb_id: 独立 Season ID                 └── tmdb_id: 继承自 Season/Show
                                                   或使用 Show ID
```

**说明**：
- **IMDB ID**: 
  - 普通电视剧：所有层级共享同一个 Show 级别的 IMDB ID（固定不变）
  - 单元剧（Anthology Series）：每个 Season 有独立的 IMDB ID，需要单独处理
- **TMDB ID**: 
  - Season Level: 使用独立 Season ID，若为 0 则回退到 Show ID
  - Episode Level: 继承自所属 Season 的 TMDB ID

**ID 继承规则**:
1. **IMDB ID**: 
   - 普通电视剧：所有层级共享同一个 Show 级别的 IMDB ID
   - 单元剧：Season Level 使用独立的 Season IMDB ID
2. **TMDB ID**: 
   - Season Level: 使用独立 Season ID，若为 0 则回退到 Show ID
   - Episode Level: 继承自所属 Season 的 TMDB ID

#### 4.4.5.1 Season 文件夹处理逻辑（2026-06-23 更新）

**核心原则**：Season 文件夹只保留 season 编号，其他所有信息必须从 API 获取。

**Season 文件夹命名规范**：
```
[S04][Season 04]                      # 只需包含 Season 编号，其他信息从 API 获取
[S04][Season 04]-[Title]             # 可选保留标题（仅用于显示，解析时忽略）
[S04][Season 04]-[Title]-tmdb450504  # 可选保留 TMDB ID（解析时忽略）
```

**解析逻辑**：
1. **只提取 season 编号**：通过正则 `[S(\d+)]` 提取 `[S04]` 中的数字 `4`
2. **忽略所有 ID**：无论是 IMDB ID 还是 TMDB ID，都不保留
3. **忽略所有标题信息**：标题、年份等信息必须从 API 获取
4. **TV Show 信息获取**：
   - 如果 TV Show 文件夹中有 TMDB ID，优先使用
   - 如果 TMDB ID 是 Season ID（导致 404），则通过标题搜索获取正确的 TV Show ID
   - 如果搜索年份无结果，尝试 `year±1` 误差查询

**实现位置**:
- `src/core/parser.rs` - `is_season_folder()` 和 `parse_season_folder_number()` 函数
- `src/core/planner.rs` - `find_tv_series_folder_context()` 函数

**关键代码**：
```rust
// Season 文件夹识别：只匹配 [SXX][Season XX] 格式
pub fn is_season_folder(dirname: &str) -> bool {
    dirname.starts_with("[S") && dirname.contains("][Season ")
}

// Season 编号提取：只提取数字，忽略其他所有信息
pub fn parse_season_folder_number(dirname: &str) -> Option<u16> {
    if let Ok(re) = regex::Regex::new(r"^\[S(\d+)\]\[Season \d+\]") {
        if let Some(caps) = re.captures(dirname) {
            if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
                return Some(num);
            }
        }
    }
    None
}
```

**验证失败处理**：
- 当所有 API 查询都失败时，文件会被标记为 `Unknown/failed`
- 在 `plan` 子命令执行后，会输出失败列表供用户查看

---

### 4.4.5.2 单元剧（Anthology Series）特殊处理

**定义**：单元剧是指每一季有独立故事线、角色和制作团队的电视剧，例如《爱，死亡和机器人》、《黑镜》等。这类剧集的每个 Season 都有独立的 IMDB ID。

**IMDB ID 优先级规则**（从高到低）：
1. **TMDB Season External IDs**：通过 TMDB API `/tv/{series_id}/season/{season_number}/external_ids` 获取的 Season 级别 IMDB ID
2. **TV Show IMDB ID**：回退到 TV Show 级别的 IMDB ID（普通电视剧共享 Show 级别 ID）

**重要提示（TMDB数据源限制）**：

| 剧集 | Season级别IMDB ID状态 | 说明 |
|------|----------------------|------|
| 爱，死亡和机器人 | Season 1-3有，Season 4无 | TMDB尚未收录Season 4的IMDB ID |
| 黑镜 | 所有Season都无 | TMDB未收录任何Season级IMDB ID |
| 普通电视剧 | 所有Season共享Show级别ID | 正常行为 |

**常见问题排查**：
- **问题**：单元剧Season文件夹使用了Show级别IMDB ID而非独立ID
- **原因**：TMDB API未返回该Season的IMDB ID（数据缺失）
- **解决方案**：等待TMDB社区更新数据，或手动编辑生成的文件夹名

**重要提醒（Tips）**：

> ⚠️ **TMDB数据源限制提醒**
> 
> | 剧集 | 单元剧类型 | Season级别IMDB ID状态 |
> |------|-----------|----------------------|
> | 爱，死亡和机器人 | 是 | Season 1-3有，Season 4 **TMDB未收录** |
> | 黑镜 | 是 | 所有Season都没有（**TMDB未收录**） |
> | 普通电视剧 | 否 | 所有Season共享Show级别ID（正常行为） |
> 
> **结论**：单元剧的Season级别IMDB ID可用性完全取决于TMDB社区的数据贡献。

**实现位置**: 
- `src/core/planner.rs` - Season IMDB ID获取逻辑
- `src/services/tmdb.rs` - `get_season_external_ids` 函数

**问题根源分析**:

文件夹名中的 `tmdb450504` 实际上是 **Season 4 的 TMDB ID**，而不是 TV Show 的 TMDB ID。当使用这个 ID 查询 TV Show 信息时，会收到 404 错误。

**当前处理流程**:

```
文件夹 TMDB ID (450504)
        │
        ▼
 尝试调用 TV Show API
        │
        ├─► 404 错误
        │         │
        │         ▼
        │   通过标题搜索获取 TV Show ID (86831)
        │         │
        │         ▼
        │   获取 TV Show 元数据 (含正确的 IMDB ID)
        │         │
        │         ▼
        │   使用 TV Show ID + Season Number 获取 Season 元数据
        │         │
        │         ▼
        └─► 成功获取 Season details

---

#### 4.4.5.3 代码优化：提取公共搜索函数

为消除 Movie 和 TV 搜索逻辑中的冗余代码，提取了公共的 URL 构建函数：

**位置**: `src/services/tmdb.rs`

```rust
/// Build URL with custom language parameter for localized search results.
/// This is a helper function to avoid code duplication between movie and TV search.
fn build_search_url(&self, endpoint: &str, query: &str, year_param: &str, language: &str) -> String {
    let api_key_param = if self.config.use_bearer {
        String::new()
    } else {
        format!("api_key={}&", self.config.api_key)
    };
    format!(
        "{}/{}?{}{}&language={}{}",
        TMDB_BASE_URL, endpoint, api_key_param, 
        format!("query={}", urlencoding::encode(query)),
        language,
        year_param
    )
}
```

**优化效果**:
- 消除了 `search_movie_with_language` 和 `search_tv_with_language` 之间约 15 行重复代码
- 统一了 URL 构建逻辑，便于维护和修改
- 提高了代码可读性和一致性

#### 4.4.5.4 代码优化：函数参数结构体化

为解决 `generate_target_info` 函数参数过多（8个参数）的问题，引入了参数结构体：

**位置**: `src/core/planner.rs`

```rust
/// Parameters for generate_target_info function
#[derive(Debug)]
struct GenerateTargetInfoParams<'a> {
    video: &'a VideoFile,
    movie_metadata: &'a Option<MovieMetadata>,
    tv_series_metadata: &'a Option<(TvSeriesMetadata, Option<EpisodeMetadata>, Option<SeasonMetadata>)>,
    parsed: &'a ParsedFilename,
    video_metadata: &'a VideoMetadata,
    target: &'a Path,
    media_type: MediaType,
}
```

**使用示例**:

```rust
// 重构前（8个参数）
let (target_info, operations, poster_download) = self.generate_target_info(
    video,
    &Some(movie_metadata.clone()),
    &None,
    &parsed,
    &video_metadata,
    target,
    media_type,
)?;

// 重构后（1个参数结构体）
let params = GenerateTargetInfoParams {
    video,
    movie_metadata: &Some(movie_metadata.clone()),
    tv_series_metadata: &None,
    parsed: &parsed,
    video_metadata: &video_metadata,
    target,
    media_type,
};
let (target_info, operations, poster_download) = self.generate_target_info(&params)?;
```

**优化效果**:
- 消除了 `clippy::too_many_arguments` 警告
- 提高了代码可读性，参数含义一目了然
- 便于扩展和维护，添加新参数只需修改结构体
- 所有7个调用点已更新为使用新的参数结构体

### 4.4.6 ID 搜索与验证逻辑

程序使用完整的 fallback 搜索逻辑来处理 ID 匹配问题，包括目录中 TMDB ID 错误的情况。

#### 4.4.6.1 核心流程

```
┌─────────────────────────────────────────────────────────────────────┐
│                    文件名字符串                                     │
└───────────────────────────┬─────────────────────────────────────────┘
                            │
                            ▼
                 ┌──────────────────────┐
                 │   guessit 解析       │
                 │ (标题、年份、ID等)    │
                 └──────────┬───────────┘
                            │
          ┌─────────────────┼─────────────────┐
          ▼                 ▼                 ▼
    解析出IMDB ID      解析出TMDB ID      解析出标题/年份
          │                 │                 │
          ▼                 ▼                 ▼
    ┌──────────┐    ┌──────────┐             │
    │ Step 1:  │    │ Step 1:  │             │
    │ 用TMDB ID │    │ 用TMDB ID │             │
    │ 搜索API   │    │ 搜索API   │             │
    └────┬─────┘    └────┬─────┘             │
         │               │                    │
         │    ┌──────────┴──────────┐         │
         │    │                     │         │
         │    ▼                     ▼         │
         │ ┌──────────┐    ┌──────────────┐   │
         │ │ 成功     │    │ 失败/数据无效 │   │
         │ │ 返回结果 │    │             │   │
         │ └──────────┘    │             │   │
         │                 └──────┬──────┘   │
         │                        │          │
         │    ┌───────────────────┴──────────┘
         │    │
         │    ▼
         │ ┌──────────────────────────────┐
         │ │ Step 2: 有IMDB ID?          │
         │ │   使用 /find/{imdb_id} API   │
         │ │   获取正确的 TMDB ID         │
         │ └──────────────┬───────────────┘
         │                │
         │    ┌───────────┴───────────┐
         │    ▼                       ▼
         │ ┌──────────┐    ┌──────────────────┐
         │ │ 成功     │    │ 失败              │
         │ │ 获取结果 │    │                  │
         │ └────┬─────┘    │                  │
         │      │          └────────┬─────────┘
         │      │                   │
         │      └───────┬───────────┘
         │              ▼
         │ ┌──────────────────────────────┐
         │ │ Step 3: 使用标题+年份搜索    │
         │ │   search_tv / search_movie  │
         │ └──────────────┬───────────────┘
         │                │
         │    ┌───────────┴───────────┐
         │    ▼                       ▼
         │ ┌──────────┐    ┌──────────────────┐
         │ │ 有匹配   │    │ 无匹配            │
         │ └────┬─────┘    └──────────────────┘
         │      │
         │      ▼
         │ ┌──────────────────────────────┐
         │ │ Step 4: 名字比对验证        │
         │ │   - 标题相似度计算          │
         │ │   - 年份验证 (±1年误差)     │
         │ │   - 选择最佳匹配结果        │
         │ └──────────────┬───────────────┘
         │                │
         │    ┌───────────┴───────────┐
         │    ▼                       ▼
         │ ┌──────────┐    ┌──────────────────┐
         │ │ 匹配成功 │    │ 匹配失败          │
         │ └────┬─────┘    └──────────────────┘
         │      │
         │      ▼
         │ ┌──────────────────────────────┐
         │ │ 以 API 返回结果为准更新元数据│
         │ │ 并获取完整的 IMDB/TMDB ID    │
         │ └──────────────────────────────┘
```

#### 4.4.6.2 名字比对算法

名字比对使用以下规则：

| 条件 | 评分 | 说明 |
|------|------|------|
| 标题完全匹配 + 年份匹配 | 1.0 | 最佳匹配 |
| 标题部分匹配 + 年份匹配 | 0.75 | 高匹配 |
| 标题部分匹配 + 年份不匹配 | 0.6 | 中等匹配 |
| 其他情况 | 0.0 | 不匹配 |

**比对逻辑**：
1. 规范化处理（去除特殊字符、转小写）
2. 计算标题包含关系（交集/最小集合 >= 60%）
3. 验证年份是否匹配（允许±1年误差）

#### 4.4.6.3 代码实现

核心函数定义在 `src/utils/metadata.rs`:

```rust
pub fn compare_titles(
    parsed_title: &str,
    parsed_year: Option<u16>,
    api_title: &str,
    api_original_title: Option<&str>,
    api_year: Option<u16>,
) -> TitleSimilarity
```

Fallback 搜索逻辑定义在 `src/core/planner.rs`:

```rust
// TV: get_tv_show_with_fallback()
// Movie: get_movie_with_fallback()
```

#### 4.4.6.4 示例

| 场景 | 目录中的 TMDB ID | IMDB ID | 结果 |
|------|-----------------|---------|------|
| TMDB ID 正确 | 86831 ✅ | 有/无 | 直接使用 86831 |
| TMDB ID 错误，有 IMDB | ~~450504~~ ❌ | tt9561862 ✅ | 通过 IMDB 获取正确 TMDB |
| TMDB ID 错误，无 IMDB | ~~450504~~ ❌ | 无 | 通过标题搜索找到正确 TMDB |
| TMDB ID 错误，标题模糊 | ~~999999~~ ❌ | 无 | 搜索结果无法匹配，返回失败 |

#### 4.4.6.5 标题搜索的多层 Fallback 机制（2026-06-20 更新）

当 TMDB ID 和 IMDB ID 都无法获取元数据时，系统会启动标题搜索的多层 fallback 机制。

**重要更新（2026-06-20）**: 为确保 TV 和 Movie 的 fallback 逻辑一致性，系统已重构为使用公共的 trait 抽象和 fallback 函数。

---

##### 4.4.6.5.1 MediaSearch Trait 抽象设计

**设计目标**: 统一 TV 和 Movie 的搜索逻辑，避免代码重复，确保一致性。

**实现位置**: `src/core/planner.rs` - `MediaSearch` trait

```rust
/// Trait for abstracting media search operations across different media types (TV/Movie).
trait MediaSearch {
    type SearchItem: Clone;

    /// Search for media items by title and optional year.
    async fn search(&self, tmdb: &TmdbClient, title: &str, year: Option<u16>) -> Result<Vec<Self::SearchItem>>;

    /// Get the display title from a search item.
    fn get_title(item: &Self::SearchItem) -> &str;

    /// Get the original title from a search item.
    fn get_original_title(item: &Self::SearchItem) -> &str;

    /// Get the year from a search item.
    fn get_year(item: &Self::SearchItem) -> Option<u16>;

    /// Get the TMDB ID from a search item.
    fn get_id(item: &Self::SearchItem) -> u64;
}
```

**Trait 实现**:

| 媒体类型 | 实现结构 | SearchItem 类型 | 标题字段 | 原始标题字段 | 年份字段 |
|---------|---------|----------------|---------|------------|---------|
| TV | `TvMediaSearch` | `TvSearchItem` | `name` | `original_name` | `first_air_date` |
| Movie | `MovieMediaSearch` | `MovieSearchItem` | `title` | `original_title` | `release_date` |

---

##### 4.4.6.5.2 公共的 search_with_fallback 函数

**核心函数**: `search_with_fallback<M: MediaSearch>()`

此函数实现了统一的多层 fallback 搜索策略，TV 和 Movie 共用此逻辑。

**Fallback 层级**:

```
search_with_fallback()
  │
  ├─► Step 1: 尝试主标题 + 年份过滤
  │     │
  │     └─► 无结果？
  │           │
  │           ├─► Step 2: 尝试原始标题 + 年份过滤
  │           │     │
  │           │     └─► 无结果？
  │           │           │
  │           │           ├─► Step 3: 尝试原始标题（无年份）
  │           │           │     │
  │           │           │     └─► 有结果 → 进入匹配验证
  │           │           │
  │           │           └─► 无结果 → 返回 None
  │
  └─► 有结果 → 进入匹配验证
        │
        ├─► Step 4a: 用搜索标题进行相似度匹配
        │     │
        │     └─► 匹配失败？
        │           │
        │           ├─► Step 4b: 用原始标题进行相似度匹配
        │           │     │
        │           │     └─► 匹配成功 → 返回最佳匹配
        │           │
        │           └─► 匹配失败 → 返回 None
        │
        └─► 匹配成功 → 返回最佳匹配
```

**关键改进点**:

| 改进 | 说明 | 解决的问题 |
|------|------|-----------|
| 中英文标题自动切换 | 中文搜索失败 → 自动尝试英文标题 | 中文标题在 TMDB 中可能无结果 |
| 年份过滤智能放宽 | 带年份搜索失败 → 自动尝试无年份 | 文件夹年份可能与实际年份不匹配 |
| 标题匹配双重验证 | 搜索标题匹配失败 → 尝试原始标题匹配 | 提高匹配成功率 |

---

##### 4.4.6.5.3 TV 和 Movie 的统一调用

**TV 调用示例** (`get_tv_show_with_fallback`):

```rust
// Step 3: Try title search using common fallback logic
let tv_search = TvMediaSearch;
let match_result = search_with_fallback(&tv_search, tmdb, title, original_title, year)
    .await
    .map_err(|e| format!("Search failed: {}", e))?;

if let Some(best_result) = match_result {
    let correct_tmdb_id = best_result.id;
    // 使用 correct_tmdb_id 获取详情...
}
```

**Movie 调用示例** (`get_movie_with_fallback`):

```rust
// Step 3: Try title search using common fallback logic
let movie_search = MovieMediaSearch;
let match_result = search_with_fallback(&movie_search, tmdb, title, original_title, year)
    .await
    .map_err(|e| format!("Search failed: {}", e))?;

if let Some(best_result) = match_result {
    let correct_tmdb_id = best_result.id;
    // 使用 correct_tmdb_id 获取详情...
}
```

---

##### 4.4.6.5.4 实际案例

| 剧集 | 问题 | 解决方案 | 结果 |
|------|------|---------|------|
| 爱，死亡和机器人 | TMDB ID 450504 返回 404 | 中文标题搜索失败 → 英文标题搜索成功 → 无年份搜索返回结果 | ✅ TMDB ID 71738 |
| 终极名单 | TMDB ID 186250 返回 404 | 中文标题搜索有结果但匹配失败 → 英文标题匹配成功 | ✅ TMDB ID 正确获取 |

---

##### 4.4.6.5.5 重构收益

| 维度 | 重构前 | 重构后 |
|------|--------|--------|
| **代码重复** | TV 和 Movie 各有一套独立逻辑 | 公共 trait + 统一函数 |
| **一致性** | TV 有完整 fallback，Movie 缺失 | TV 和 Movie 行为完全一致 |
| **可维护性** | 新增 fallback 层需同步修改两处 | 只需修改一处公共函数 |
| **可测试性** | 需分别测试 TV 和 Movie | 公共逻辑可单独测试 |
| **扩展性** | 新增媒体类型需复制整套逻辑 | 只需实现 MediaSearch trait |

### 4.5 排序前缀生成规则

| 优先级 | 条件 | 规则 |
|-------|------|------|
| 1 | 标题包含中文 | 使用中文标题的拼音首字母 |
| 2 | 原始语言为英语 | 移除 The/A/An 后取首字母 |
| 3 | 其他语言 | 直接使用标题首字母 |

```rust
pub fn generate_sort_prefix(title: &str, original_language: &str) -> char {
    // 规则 1: 如果标题包含中文字符，使用拼音
    if chinese::contains_chinese(title) {
        return chinese::get_first_pinyin_letter(title);
    }
    
    // 规则 2: 英语 - 先移除冠词
    if original_language == "en" {
        let effective_title = remove_articles(title);
        return effective_title.chars().next().unwrap_or('?').to_ascii_uppercase();
    }
    
    // 规则 3: 其他语言 - 直接使用首字符
    title.chars().next().unwrap_or('?').to_ascii_uppercase()
}
```

### 4.5 语言文件夹结构

```
TV_00_TMP/
└── EN_English/
    └── [{排序前缀}][{标题}][{原始标题}]({年份})-tmdb{id}/
        └── [{排序前缀}][{标题}][{原始标题}]({年份})-{imdb_id}-tmdb{id}/
            └── [S{季号}][Season {季号}]-...
                └── {集号}.{集名}.{格式}
```

---

## 5. 文件命名规范

### 5.1 电影文件格式

```
[{排序前缀}][{标题}][{原始标题}]({年份})-{分辨率}({清晰度})-{编码}-{比特深度}.{扩展名}
```

**示例**:
```
[N][女人的碎片][Pieces of a Woman](2020)-1920x1080(1080p)-h264-8bit-aac-2.0.mp4
[T][特工迷阵][The Wrecking Crew](2026)-3840x1600(2160p)-WEB-DL-hevc-10bit-eac3-5.1.mkv
```

### 5.2 电视剧文件格式

```
[{集号}][-{集名}][-{排序前缀}][{标题}][{原始标题}]-{分辨率}({清晰度})-{编码}-{比特深度}.{扩展名}
```

**示例**:
```
[S04E01]-[停不下来！]-[L][Love, Death & Robots]-1920x1080(1080p)-h264-8bit-eac3-5.1.mp4
[S01E01]-[56 Days]-[T][56 Days]-1920x804(1080p)-WEB-DL-hevc-10bit-eac3-5.1.mkv
```

---

## 5. Movies 与 TV 的区别对比

### 5.1 核心差异概览

| 特性 | Movies（电影） | TV Series（电视剧） |
|------|---------------|---------------------|
| **结构层次** | 单层结构 | 三层结构（Show → Season → Episode） |
| **文件数量** | 通常单文件 | 多文件（每集一个文件） |
| **命名复杂度** | 相对简单 | 复杂（需包含季号、集号） |
| **ID 处理** | 单一 IMDB/TMDB ID | Show/Season 两层 ID |
| **IMDB Season ID** | 不适用 | 不存在（共用 Show ID） |
| **TMDB Season ID** | 不适用 | 存在但可能为 0 |

### 5.2 文件夹结构对比

**Movies**:
```
Movies_00_TMP/
└── EN_English/
    └── [N][女人的碎片][Pieces of a Woman](2020)-tt11161474-tmdb641662/
        ├── poster.jpg
        ├── movie.nfo
        └── [N][女人的碎片][Pieces of a Woman](2020)-1920x1080(1080p)-h264-8bit-aac-2.0.mp4
```

**TV Series**:
```
TV_00_TMP/
└── EN_English/
    └── [A][爱，死亡和机器人][Love, Death & Robots](2025)-tt9561862-tmdb450504/
        ├── poster.jpg
        ├── tvshow.nfo
        └── [S04][Season 04]-[A][爱，死亡和机器人][Love, Death & Robots]-tt9561862-tmdb450504/
            ├── season.nfo
            └── S04E01-[停不下来！]-[L][Love, Death & Robots]-1920x1080(1080p)-h264-8bit-eac3-5.1.mp4
```

### 5.3 文件命名规则对比

**Movies**:
```
[{排序前缀}][{中文标题}][{原始英文标题}]({年份})-{分辨率}({清晰度})-{编码}-{比特深度}.{扩展名}
```

**TV Series**:
```
[{集号}][-{集名}][-{排序前缀}][{标题}][{原始标题}]-{分辨率}({清晰度})-{编码}-{比特深度}.{扩展名}
```

### 5.4 关键处理差异

#### 5.4.1 ID 处理
- **Movies**: 直接使用文件/目录中的 IMDB/TMDB ID
- **TV Series**: 需要区分 Show Level 和 Season Level 的 ID，处理 Season ID 为 0 的情况

#### 5.4.2 标题提取
- **Movies**: 主要从文件名或目录名提取
- **TV Series**: 需从 TV Show 文件夹、Season 文件夹、文件名多层提取，优先级：目录名 > TMDB API

#### 5.4.3 文件夹生成
- **Movies**: 单层文件夹生成
- **TV Series**: 三层嵌套文件夹生成，需要传递多层元数据

#### 5.4.4 并发处理
- **Movies**: 单文件处理，并发相对简单
- **TV Series**: 多季多集处理，需考虑同一 Show 下文件的关联性

### 5.5 需要关注的关键点

#### 对于 Movies
1. **文件名解析**: 正确提取年份、分辨率、编码等信息
2. **中文译名**: 通过 TMDB Translations API 获取 CN > SG > HK > TW 优先级
3. **NFO 生成**: 包含演员、导演、剧情简介等信息

#### 对于 TV Series
1. **层级关系**: 正确识别 Show → Season → Episode 的嵌套关系
2. **ID 继承**: 处理 Season ID 缺失时的回退逻辑
3. **非标准文件夹**: 支持识别非标准格式的 TV Show 文件夹（如包含 "Season" 关键词）
4. **英文原名**: 从非标准文件夹名中提取英文原始标题
5. **季号集号**: 正确解析文件名中的 SxxExx 格式
6. **IMDB ID 获取**: 优先级为目录名 > TMDB API，若两者都没有则文件夹名不包含 IMDB ID

#### 共同关注点
1. **缓存机制**: 合理使用会话缓存和磁盘缓存减少 TMDB API 调用
2. **错误处理**: TMDB API 返回空数据时的备用方案
3. **命名规范化**: 统一的文件名清理和排序前缀生成规则
4. **多线程并发**: 使用 Tokio 异步处理提升性能

---

## 6. TMDB ID 提取与处理

### 6.1 文件名中的 ID 格式

```
tmdb{数字}  - TMDB ID
tt{数字}    - IMDB ID
```

**示例**:
```
S04E01-[停不下来！]-[L][Love, Death & Robots]-1920x1080(1080p)-h264-8bit-eac3-5.1.mp4
                                                              ^^^^^^^^ tmdb450504
```

### 6.2 IMDB 与 TMDB 的 Season ID 区别

| 特性 | IMDB | TMDB |
|------|------|------|
| **Season Level ID** | ❌ 不存在 | ✅ 存在 |
| **Show Level ID** | ✅ 存在 | ✅ 存在 |
| **说明** | 整季共用一个 IMDB ID | 每季有独立的 TMDB ID |

**技术细节**:
- **IMDB**: 电视剧只有一个统一的 IMDB ID（如 `tt9561862`），所有季和集共享这个 ID
- **TMDB**: 电视剧有 Show 级别的 TMDB ID，同时每一季也有独立的 Season 级别的 TMDB ID

**示例**:
- Show 级别: `tmdb450504` (爱，死亡和机器人 第四季)
- Season 级别: 每季有独立 ID（TMDB API 返回）

当 TMDB API 返回的 Season ID 为 0 或缺失时，系统会使用 Show 级别的 TMDB ID 作为备用：

```rust
let effective_tmdb_id = if season.as_ref().map_or(true, |s| s.tmdb_id == 0) {
    show.tmdb_id  // 使用 Show 级别的 ID
} else {
    season.as_ref().map(|s| s.tmdb_id).unwrap_or(show.tmdb_id)
};
```

### 6.3 ID 优先级

1. **文件名中的 ID** (最高优先级)
2. **父目录名中的 ID**
3. **TMDB API 查询结果**

### 6.3 TMDB ID 为 0 的处理

当 TMDB API 返回的 Season ID 为 0 时，使用 Show 级别的 TMDB ID 作为备用:

```rust
let effective_tmdb_id = if season.as_ref().map_or(true, |s| s.tmdb_id == 0) {
    show.tmdb_id  // 使用 Show 级别的 ID
} else {
    season.as_ref().map(|s| s.tmdb_id).unwrap_or(show.tmdb_id)
};
```

---

## 7. 多线程并发处理

### 7.1 并发处理场景

| 场景 | 并发方式 |
|------|---------|
| FFprobe 视频信息提取 | Tokio 异步并行 |
| 视频文件处理 | 异步流处理 |
| TMDB ID 查询 | 并发限制 (semaphore) |
| TMDB 元数据获取 | 会话缓存 + 并发限制 |
| 海报下载 | 异步并行 |

### 7.2 并发控制

使用信号量 (Semaphore) 控制并发数量:

```rust
let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_TMDB));
let _permit = semaphore.acquire().await?;
```

### 7.3 缓存机制

- **会话缓存**: 存储 TMDB API 响应，避免重复请求
- **文件级缓存**: TMDB 响应持久化到磁盘

---

## 8. 公共函数模块清单（2026-06-20 更新）

本章节记录系统中已提取的公共函数模块，以及建议提取但尚未提取的模块。

---

### 8.1 已提取的公共函数模块

#### 8.1.1 语言/区域代码转换模块 (`utils/locale.rs`) - 新增（2026-06-20）

| 函数名 | 功能 | 使用场景 |
|--------|------|---------|
| `find_priority_chinese_title` | 中文标题优先级选择（CN > SG > HK > TW） | TMDB translations 处理 |
| `country_code_to_name` | ISO 3166-1 国家代码转名称 | NFO 元数据、文件夹命名 |
| `language_code_to_name` | ISO 639-1 语言代码转名称 | 文件夹命名 |
| `normalize_language_code` | 语言代码规范化（"cn" -> "zh"） | TMDB quirks 处理 |
| `format_language_folder` | 格式化语言文件夹名（"ZH_Chinese"） | 语言文件夹生成 |

**代码示例**:
```rust
use crate::utils::locale::{
    find_priority_chinese_title,
    country_code_to_name,
    language_code_to_name,
    format_language_folder,
};

// 中文标题优先级选择
let candidates = vec![
    ("CN".to_string(), "爱，死亡和机器人".to_string()),
    ("TW".to_string(), "愛·死·和機械人".to_string()),
];
let title = find_priority_chinese_title(&candidates);  // "爱，死亡和机器人"

// 国家代码转换
let country = country_code_to_name("US");  // "United States"

// 语言代码转换
let lang = language_code_to_name("zh");  // "Chinese"

// 语言文件夹格式化
let folder = format_language_folder("zh");  // "ZH_Chinese"
```

---

#### 8.1.2 元数据工具模块 (`utils/metadata.rs`)

| 函数名 | 功能 | 使用场景 |
|--------|------|---------|
| `normalize_title` | 标题规范化（去除特殊字符、统一格式） | 标题匹配、搜索 |
| `title_contains` | 标题包含关系判断（支持中英文） | 标题匹配验证 |
| `compare_titles` | 标题相似度比对（返回匹配分数） | TV/Movie 搜索结果匹配 |
| `is_valid_imdb_id` | IMDB ID 格式验证 | ID 提取验证 |
| `is_valid_tmdb_id` | TMDB ID 格式验证 | ID 提取验证 |
| `extract_ids_from_text` | 从文本提取 IMDB/TMDB ID | 文件名解析 |

**代码示例**:
```rust
use crate::utils::metadata::{normalize_title, compare_titles, is_valid_tmdb_id};

// 标题规范化
let normalized = normalize_title("爱，死亡和机器人");  // "爱死亡和机器人"

// 标题相似度比对
let similarity = compare_titles(
    "Love, Death & Robots",
    Some(2025),
    "Love Death & Robots",
    Some("Love, Death & Robots"),
    Some(2024),
);
// similarity.matched = true, similarity.score = 0.85

// TMDB ID 验证
if is_valid_tmdb_id("450504") {
    // 有效 ID
}
```

---

#### 8.1.3 中文处理模块 (`utils/chinese.rs`)

| 函数名 | 功能 | 使用场景 |
|--------|------|---------|
| `titles_equivalent` | 中英文标题等价判断 | 标题匹配 |
| `normalize` | 中文文本规范化 | 标题处理 |
| `contains_chinese` | 检测是否包含中文 | 语言判断 |
| `get_first_pinyin_letter` | 获取拼音首字母 | 排序前缀生成 |

**代码示例**:
```rust
use crate::utils::chinese::{contains_chinese, get_first_pinyin_letter};

// 中文检测
if contains_chinese("爱，死亡和机器人") {
    // 包含中文
}

// 拼音首字母
let letter = get_first_pinyin_letter("爱，死亡和机器人");  // 'A'
```

---

#### 8.1.4 哈希工具模块 (`utils/hash.rs`)

| 函数名 | 功能 | 使用场景 |
|--------|------|---------|
| `sha256_file` | 计算文件 SHA256 哈希 | 文件唯一性验证 |
| `sha256_string` | 计算字符串 SHA256 哈希 | ID 生成 |

---

#### 8.1.5 文件系统工具模块 (`utils/fs.rs`)

| 函数名 | 功能 | 使用场景 |
|--------|------|---------|
| `ensure_directory` | 确保目录存在 | 目录创建 |
| `create_dir_all` | 创建多级目录 | 目录创建 |
| `move_file` | 移动文件（支持跨设备） | 文件整理 |
| `get_extension` | 获取文件扩展名 | 文件类型判断 |
| `is_video_file` | 判断是否为视频文件 | 文件扫描 |
| `is_sample` | 判断是否为样本文件 | 样本文件过滤 |

---

#### 8.1.6 NFO 生成模块 (`generators/nfo.rs`)

| 函数名 | 功能 | 使用场景 |
|--------|------|---------|
| `generate_movie_nfo` | 生成电影 NFO 文件 | Kodi/Plex 元数据 |
| `generate_tv_series_nfo` | 生成 TV 剧集 NFO | Kodi/Plex 元数据 |
| `generate_episode_nfo` | 生成单集 NFO | Kodi/Plex 元数据 |
| `generate_season_nfo` | 生成季 NFO | Kodi/Plex 元数据 |

---

#### 8.1.7 文件名生成模块 (`generators/filename.rs`)

| 函数名 | 功能 | 使用场景 |
|--------|------|---------|
| `extract_disc_identifier` | 提取光盘标识（如 `CD1`, `Part1`） | 多文件电影 |
| `generate_movie_filename` | 生成电影文件名 | 电影整理 |
| `generate_movie_filename_with_disc` | 生成带光盘标识的电影文件名 | 多文件电影 |
| `generate_episode_filename` | 生成单集文件名 | TV 整理 |

---

#### 8.1.8 文件夹生成模块 (`generators/folder.rs`)

| 函数名 | 功能 | 使用场景 |
|--------|------|---------|
| `generate_sort_prefix` | 生成排序前缀（拼音首字母） | 文件夹排序 |
| `generate_movie_folder` | 生成电影文件夹名 | 电影整理 |
| `generate_tv_series_folder` | 生成 TV 剧集文件夹名 | TV 整理 |
| `generate_season_folder` | 生成季文件夹名 | TV 整理 |

---

#### 8.1.9 媒体搜索抽象模块 (`planner.rs` - 新增)

| Trait/函数 | 功能 | 使用场景 |
|-----------|------|---------|
| `MediaSearch` trait | 媒体搜索抽象接口 | TV/Movie 统一搜索 |
| `TvMediaSearch` | TV 搜索实现 | TV 剧集搜索 |
| `MovieMediaSearch` | Movie 搜索实现 | 电影搜索 |
| `search_with_fallback` | 公共 fallback 搜索函数 | TV/Movie 共用 |

**设计亮点**: 使用 trait 抽象实现 TV 和 Movie 的搜索逻辑统一，避免代码重复。

---

### 8.2 建议提取但尚未提取的模块

以下模块目前为私有函数或分散在多个文件中，建议提取为公共函数模块：

| 模块名称 | 当前位置 | 建议提取为 | 提取原因 | 审阅结论 |
|---------|---------|-----------|---------|---------|
| **语言/国家代码转换** | ~~`planner.rs` (私有函数)~~ | ~~`utils/locale.rs`~~ | ~~`language_code_to_name`, `country_code_to_name` 被 TV/Movie/NFO 多处调用~~ | ✅ **已提取** (2026-06-20) |
| **元数据构建器** | `planner.rs` (私有方法) | `utils/metadata_builder.rs` 或 trait | `build_tv_series_metadata_from_details` 和 `build_movie_metadata_from_details` 逻辑相似 | ❌ 不建议提取（依赖上下文、复杂业务逻辑） |
| **目录解析器** | `core/metadata.rs` | `utils/path_parser.rs` | `parse_organized_directory` 解析逻辑复杂，应独立模块 | ❌ 不建议提取（已在专门模块中、无跨模块调用） |
| **TMDB API 重试** | `services/tmdb.rs` (私有方法) | `utils/api.rs` 或 trait | `request_with_retry` 重试逻辑可被其他 API 服务复用 | ❌ 不建议提取（依赖上下文、无其他API服务） |

**审阅结论说明**:

- ✅ **已提取**: 语言/国家代码转换模块已成功提取到 `utils/locale.rs`，包含 5 个公共函数和完整 UT 测试覆盖
- ❌ **不建议提取**: 其他 3 个模块经审阅后，确认不适合提取为公共模块，理由详见各模块审阅结论

---

### 8.3 公共函数模块设计原则

提取公共函数时应遵循以下原则：

| 原则 | 说明 |
|------|------|
| **DRY** | Don't Repeat Yourself - 避免代码重复 |
| **单一职责** | 每个模块只负责一个功能领域 |
| **可测试性** | 公共函数应易于单独测试 |
| **一致性** | TV 和 Movie 的相似逻辑应统一 |
| **扩展性** | 新增媒体类型时只需实现差异部分 |

---

## 9. 常见问题与最佳实践

### 9.1 TMDB API 返回空数据

**问题**: TMDB API 返回的 `original_language` 或 `original_name` 为空

**解决方案**:
```rust
let original_language = details.original_language.clone().unwrap_or_else(|| {
    // 基于 origin_country 推断语言代码
    if let Some(ref countries) = details.origin_country {
        if !countries.is_empty() {
            return match countries[0].to_uppercase().as_str() {
                "US" | "GB" | "AU" | "CA" => "en".to_string(),
                "CN" | "HK" | "TW" | "MO" => "zh".to_string(),
                "JP" => "ja".to_string(),
                "KR" => "ko".to_string(),
                _ => countries[0].to_lowercase(),
            };
        }
    }
    "en".to_string()  // 默认使用英语
});
```

### 9.2 非标准 TV 文件夹识别

**问题**: TV 文件夹不符合标准格式，如 `爱，死亡和机器人 第四季 Love, Death & Robots Season 4 (2025)`

**解决方案**: 通过关键词识别非标准文件夹:

```rust
let is_likely_tvshow_folder = name.contains("Season") 
    || name.contains("Love, Death & Robots") 
    || name.contains("Terminal List");
```

### 9.3 英文原名提取

**问题**: 从非标准文件夹名中提取英文原名

**解决方案**:
```rust
pub fn extract_english_title_from_dirname(dirname: &str) -> Option<String> {
    // 移除中文字符，保留英文、标点和空格
    let cleaned: String = before_season.chars().filter(|c| {
        c.is_ascii_alphabetic() || c.is_ascii_whitespace() 
            || *c == '&' || *c == ',' || *c == '\'' || *c == '-' || *c == ':'
    }).collect();
    
    // 清理并返回英文标题
    Some(parts.join(" "))
}
```

---

## 10. 总结

Media Organizer 通过以下方式实现高效的媒体文件整理:

1. **智能中文译名获取**: 遵循 CN > SG > HK > TW 优先级
2. **规范化命名**: 统一的文件夹和文件命名格式
3. **多线程并发**: 使用 Tokio 实现高效的异步处理
4. **缓存机制**: 会话缓存和磁盘缓存减少 API 调用
5. **容错处理**: 多种备用方案确保元数据获取成功

---

*文档版本: 1.0*
*最后更新: 2026-06-19*
