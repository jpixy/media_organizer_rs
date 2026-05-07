# Media Organizer 项目实施计划

## ✅ 核心原则
- 按顺序开发，每个阶段完成后再进入下一阶段
- 每个阶段必须包含单元测试，覆盖率 ≥70%
- 后开发模块不能推翻前序模块接口
- 初级工程师可按本文档独立完成全部开发

---

## 🎯 开发阶段划分

| 阶段 | 模块 | 预计工作量 | 依赖 | 测试要求 |
|------|------|-----------|------|----------|
| 🔹 阶段 1 | 数据模型层 | 3 天 | 无 | 100% 覆盖率 |
| 🔹 阶段 2 | 工具与服务层 | 5 天 | 阶段1 | ≥70% 覆盖率 |
| 🔹 阶段 3 | 核心业务层 | 7 天 | 阶段1, 阶段2 | ≥70% 覆盖率 |
| 🔹 阶段 4 | CLI 入口层 | 3 天 | 阶段3 | 集成测试 |
| 🔹 阶段 5 | 高级功能 | 5 天 | 阶段4 | ≥70% 覆盖率 |

---

## 🔹 阶段 1: 数据模型层 (首先完成)

### ✅ 必须实现的结构体
```rust
// src/models/mod.rs

pub struct MediaFile {
    path: PathBuf,
    size: u64,
    file_hash: Option<String>,
}

pub struct CandidateMetadata {
    chinese_title: Option<String>,
    english_title: Option<String>,
    year: Option<u16>,
    season: Option<u32>,
    episode: Option<u32>,
    tmdb_id: Option<u64>,
    imdb_id: Option<String>,
    confidence: f32,
    source: MetadataSource,
}

pub enum MetadataSource {
    Filename,
    Directory,
    TMDB,
    AIEnhanced,
}

pub struct Plan {
    version: String,
    created_at: DateTime<Utc>,
    media_type: MediaType,
    source_path: PathBuf,
    items: Vec<PlanItem>,
    unknown: Vec<UnknownItem>,
    samples: Vec<SampleItem>,
}

pub struct PlanItem {
    id: Uuid,
    source: MediaFile,
    metadata: CandidateMetadata,
    target_folder: PathBuf,
    target_filename: String,
    operations: Vec<Operation>,
}

pub enum Operation {
    Mkdir { path: PathBuf },
    Move { from: PathBuf, to: PathBuf },
    CreateNfo { path: PathBuf, content: String },
    DownloadPoster { url: String, path: PathBuf },
}

pub struct Rollback {
    plan_id: Uuid,
    executed_at: DateTime<Utc>,
    operations: Vec<ExecutedOperation>,
}

pub struct ExecutedOperation {
    seq: u32,
    op_type: OperationType,
    success: bool,
}
```

### ✅ 单元测试要求
- 每个结构体序列化/反序列化测试
- JSON 格式兼容性测试
- 边界值测试
- ✅ **必须 100% 测试覆盖率**

---

## 🔹 阶段 2: 工具与服务层

### ✅ 必须实现的模块
| 模块 | 接口定义 | 测试要求 |
|------|---------|----------|
| **src/services/ffprobe.rs** | `fn probe(path: &Path) -> Result<VideoInfo, Error>` | Mock 测试 |
| **src/services/tmdb.rs** | `fn search_movie(title: &str, year: Option<u16>) -> Result<Vec<TmdbResult>, Error>` | Mock HTTP 测试 |
| **src/services/ollama.rs** | `fn enhance_metadata(candidate: &CandidateMetadata) -> Result<CandidateMetadata, Error>` | Mock 测试 |
| **src/utils/fs.rs** | `fn safe_move(from: &Path, to: &Path) -> Result<(), Error>` | 集成测试 |
| **src/utils/hash.rs** | `fn fast_file_hash(path: &Path) -> Result<String, Error>` | 单元测试 |
| **src/preflight/mod.rs** | `fn check_all() -> Result<(), Vec<PreflightError>>` | 单元测试 |

### ✅ 开发顺序
1. utils -> 2. services -> 3. preflight

---

## 🔹 阶段 3: 核心业务层

### ✅ 必须实现的模块与顺序
| 顺序 | 模块 | 核心函数 | 测试要求 |
|------|------|---------|----------|
| 1 | **src/core/scanner.rs** | `fn scan_directory(path: &Path, media_type: MediaType) -> Result<Vec<MediaFile>, Error>` | 临时目录测试 |
| 2 | **src/core/parser.rs** | `fn parse_filename(filename: &str) -> Result<CandidateMetadata, Error>` | ✅ 基于 hunch 库实现，至少100个测试用例，覆盖率 ≥90% |
| 3 | **src/generators/filename.rs** | `fn generate_filename(metadata: &CandidateMetadata, video_info: &VideoInfo) -> Result<String, Error>` | 单元测试 |
| 4 | **src/generators/folder.rs** | `fn generate_folder(metadata: &CandidateMetadata) -> Result<PathBuf, Error>` | 单元测试 |
| 5 | **src/core/metadata.rs** | `fn extract_metadata(file: &MediaFile) -> Result<CandidateMetadata, Error>` | 集成测试 |
| 6 | **src/services/ollama.rs** | `fn enhance_metadata(candidate: &CandidateMetadata) -> Result<CandidateMetadata, Error>` | Mock 测试 |
| 7 | **src/core/planner.rs** | `fn generate_plan(files: &[MediaFile], media_type: MediaType) -> Result<Plan, Error>` | 集成测试 |
| 6 | **src/core/executor.rs** | `fn execute_plan(plan: &Plan) -> Result<Rollback, Error>` | 集成测试 |
| 7 | **src/core/rollback.rs** | `fn rollback(rollback: &Rollback) -> Result<(), Error>` | 集成测试 |
| 8 | **src/core/indexer.rs** | `fn index_path(path: &Path) -> Result<(), Error>` | 单元测试 |

### ✅ 关键点
- 每个模块完成后必须写测试再进入下一个
- parser.rs 需要至少 100 个测试用例
- 禁止循环依赖

---

## 🔹 阶段 4: CLI 入口层

### ✅ 必须实现的子命令
```
media-organizer
  ├── plan movies <source> [--target]
  ├── plan tv_series <source> [--target]
  ├── execute <plan.json>
  ├── rollback <rollback.json> [--dry-run]
  ├── verify <path>
  ├── index scan <path>
  ├── index stats
  ├── index list
  ├── index duplicates
  ├── search <query>
  ├── export <output.zip>
  ├── import <input.zip>
  └── sessions list
```

### ✅ 实现要求
- 使用 `clap` 库
- 每个子命令只做参数解析，业务逻辑全部调用 core 层
- CLI 层不包含任何业务逻辑

---

## 🔹 阶段 5: 高级功能

| 顺序 | 功能 | 依赖 |
|------|------|------|
| 1 | 导出/导入功能 | 阶段3 完成 |
| 2 | 中央索引与搜索 | 阶段3 完成 |
| 3 | 会话历史管理 | 阶段4 完成 |
| 4 | 重复文件检测 | 索引 完成 |
| 5 | 系列集合管理 | 索引 完成 |

---

## ✅ 测试策略

### 🧪 单元测试
- 所有模型、工具、服务模块必须有单元测试
- 外部服务必须 Mock，不能依赖真实网络
- 覆盖率 ≥70%
- CI 自动运行所有测试

### 🧪 集成测试
- Plan -> Execute -> Rollback 全流程测试
- 使用临时目录，不修改真实文件
- 幂等性测试：重复执行3次结果必须一致

### 🧪 验收测试
- 输入 10 个混乱命名的测试文件
- 执行完整整理流程
- 验证输出目录结构符合规范
- 验证 rollback 可以完全恢复

---

## ⚠️ 禁止行为
1. ❌ 禁止提前开发高级功能，必须按阶段顺序
2. ❌ 禁止在底层模块中引入高层模块依赖
3. ❌ 禁止修改已经完成阶段的公共接口
4. ❌ 禁止没有测试就进入下一阶段
5. ❌ 禁止在 CI 变红的情况下继续开发

---

## 📌 里程碑检查点

| 里程碑 | 验收标准 |
|--------|----------|
| ✅ 阶段1 完成 | 所有模型 JSON 序列化稳定 |
| ✅ 阶段2 完成 | 所有外部服务可以正常调用 |
| ✅ 阶段3 完成 | Plan/Execute/Rollback 全流程可用 |
| ✅ 阶段4 完成 | 所有 CLI 命令可以正常执行 |
| ✅ 阶段5 完成 | 完整功能可以使用 |

---

## 🎯 最终交付
- 所有测试通过
- 代码覆盖率 ≥70%
- 文档完整
- 可以通过 cargo install 安装