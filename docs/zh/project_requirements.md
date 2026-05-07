# Media Organizer 项目需求说明（整合版）

## 1. 项目目标

开发一个仅命令行（CLI）的媒体整理工具。用户提供一个目录路径，该路径下只包含一种媒体类型：

- 要么全部是电影（movies）
- 要么全部是电视剧（tv_series）

工具需要完成：识别文件信息、匹配 TMDB 元数据、生成规范化目录结构、输出 NFO/海报、支持安全执行与回滚，并提供后续索引与搜索能力。

---

## 2. 核心原则

### 2.1 准确性优先

- 文件名解析准确率是第一优先级（中英文标题、年份、季集号）
- 匹配不确定时默认跳过，不强行归类（宁可遗漏，不能错误）
- 默认采用本地解析并追求最高可达准确率，AI 策略遵循 10.2 节核心原则

### 2.2 幂等性

- 对同一目录重复执行，多次结果必须一致
- 已整理内容可识别，不重复制造新结果或无意义重命名

### 2.3 安全性

- 标准流程为 `plan -> execute -> rollback`
- 所有实际文件操作必须可回滚
- 支持 dry-run/预览，默认优先安全检查

---

## 3. 输入与扫描需求

### 3.1 输入约束

- 输入目录内媒体类型由用户明确指定为 `movies` 或 `tv_series`
- 程序无需自动判断电影/剧集类型

### 3.2 目录复杂度支持

- 支持任意深度嵌套目录
- 支持目录中存在空文件夹
- 支持文件在技术目录内（例如 `4K/`、`S01/`、`WEB-DL/`）

### 3.3 智能目录解析

- 自动跳过技术子目录（分辨率、季目录、画质、编码标签）
- 自动清理排序前缀（如 `A_`、`X_`、`01_`）
- 支持从父目录补充上下文（尤其标题信息）

---

## 4. 元数据识别与匹配需求

### 4.1 文件名解析

至少提取以下信息（能提取多少取决于源文件质量）：

- 标题（中文/英文）
- 年份
- 电影版本信息（edition，可选）
- TV 的季号/集号
- 技术信息（分辨率、视频格式、视频编码、位深、音频编码、声道）

### 4.1.1 本地解析引擎策略（语言说明）

- 本项目当前技术栈为**纯 Rust**，实现时应优先选择 Rust 生态的本地解析方案（例如 `hunch` 或等价能力实现）。
- 同时在需求文档中保留 Python 生态参考方案（例如 `guessit`），用于后续对照测试、准确率基准评估和跨语言方案调研。
- 该参考不改变当前实现语言：生产实现仍以 Rust 为主，不引入 Python 运行时作为默认依赖。
- 本地解析能力必须持续优化，并作为准确率基线能力。
- AI 策略遵循 10.2 节核心原则。

### 4.2 识别优先级

元数据匹配优先级：

1. 路径中已有 TMDB ID（最高优先级）
2. 路径中已有 IMDB ID
3. 标题 + 年份搜索
4. AI 补充解析后再搜索（可选）

说明：路径包含“文件名 + 所有父目录名”。

### 4.3 TMDB 匹配策略

- 通过 `api.themoviedb.org` 获取电影/剧集详情
- 标题与年份联合验证
- 可容忍有限年份偏差（如 +/-1）但默认保守
- 低置信匹配进入 unknown 列表，不执行落盘整理

### 4.4 已整理内容识别

- 可识别已整理命名并提取其中的 TMDB/IMDB 信息
- 对增量文件（例如已整理剧集目录中新加一集）可进行增量处理

---

## 5. 命名与输出规范

### 5.1 电影目录/文件命名

✅ **业务需求（必须）**
- 电影目录必须包含唯一标识信息
- 中文电影标题不重复显示中文
- 简繁体视为同一标题语义
- edition 信息为可选字段

📌 **命名规范（实现要求）**
电影目录遵循：
- 通用格式：`[${originalTitle}]-[${title}](${- ,edition,})-${year}-${imdb}-${tmdb}`
- 中文电影优化：`[${title}](${- ,edition,})-${year}-${imdb}-${tmdb}`（仅保留中文标题）

电影文件名遵循：
- `[${originalTitle}]-[${title}](${- ,edition,})-${year}-${videoResolution}-${videoFormat}-${videoCodec}-${videoBitDepth}bit-${audioCodec}-${audioChannelsAsString}`

---

### 5.2 电视剧目录/文件命名

✅ **业务需求（必须）**
- 剧集根目录必须包含剧集唯一标识
- 季目录必须可识别季号与首播年份
- 单集文件必须包含完整识别信息

📌 **命名规范（实现要求）**
剧集根目录遵循：
- `[${showOriginalTitle}]-[${showTitle}]-${showImdb}-${showTmdb}`

分季目录遵循：
- `S${seasonNr2}.${showYear}` （例如：`S01.2020`）

单集文件遵循：
- `[${showOriginalTitle}]-S${seasonNr2}E${episodeNr2}-[${originalTitle}]-[${title}]-${videoFormat}-${videoCodec}-${videoBitDepth}bit-${audioCodec}-${audioChannelsAsString}`

### 5.3 语言层级目录

- 按原始语言组织到语言目录（如 `ZH_Chinese`、`EN_English`）

### 5.4 元数据文件与图片

- 生成通用媒体中心兼容 NFO（Kodi/Emby/Jellyfin 兼容）
- NFO 至少包含：简介、导演、演员、ID 信息
- 海报下载为建议能力：每个条目 1-3 张为佳（可配置，不强制）

### 5.5 附属文件处理

- 识别并移动字幕文件/字幕目录
- 识别并移动 Extras/Featurettes/Bonus 等附加内容
- 识别并移动 Sample 文件或 Sample 文件夹到目标条目下统一归档

---

## 6. 执行模型与事务需求

### 6.1 Plan 阶段

- 扫描源目录并生成 `plan.json`
- 明确列出将执行的操作：`mkdir / move / create / download`
- 输出 unknown 与 sample 分类结果
- 检查目标路径冲突并提前报错

### 6.2 Execute 阶段

- 严格按计划执行
- 每步写入可回滚记录
- 输出 `rollback.json`

### 6.3 Rollback 阶段

- 支持根据 `rollback.json` 逆向恢复
- 支持 rollback dry-run

### 6.4 Verify 子命令

- 提供独立 CLI 子命令校验视频完整性
- 默认整理流程不强制开启（以效率优先）

---

## 7. CLI 能力范围

### 7.1 基础整理命令

- `plan movies <source> [--target]`
- `plan tv_series <source> [--target]`
- `execute <plan.json>`
- `rollback <rollback.json> [--dry-run]`
- `verify <path>`
- `sessions list/show`（会话与 plan/rollback 历史查询）

### 7.2 预检查

默认支持 preflight 检查：

- ffprobe 可用
- Ollama 服务可用（如启用 AI）
- TMDB API 可用

可通过 `--skip-preflight` 跳过。

### 7.3 索引与搜索

✅ **业务需求（必须）**
- 支持跨硬盘中央索引系统
- 硬盘离线时仍可搜索
- 支持重复文件识别
- 支持电影系列集合管理

📌 **命令规范（实现要求）**
index 子命令体系：
- `index scan` - 扫描路径建立索引
- `index stats` - 显示索引统计信息
- `index list` - 列出索引条目
- `index verify` - 校验索引完整性
- `index remove` - 删除索引条目
- `index duplicates` - 查找重复文件
- `index collections` - 管理电影系列集合
- `index rename` - 硬盘重命名，自动更新关联索引

搜索支持字段：
- `search` 支持 title/actor/director/collection/year/genre/country
- language 字段与 country 关联映射
- 搜索结果可区分电影与电视剧，并展示硬盘在线/离线状态

> **实现策略（建议）**：
> 硬盘重命名操作应自动触发索引刷新，无需用户手动重新扫描

---

## 8. 中央索引需求

### 8.1 目标

- 即使硬盘离线也可搜索
- 记录媒体位于哪个硬盘、哪个路径
- 支持电影系列（collection）完整度统计

### 8.2 存储

位于 `~/.config/media_organizer/`：

- `central_index.json`
- `disk_indexes/*.json`
- 自动备份文件

### 8.3 索引粒度

- 电影与电视剧均入索引
- 支持同一 disk-label 同时挂电影和剧集路径
- 支持重复副本展示，不强制去重删除

---

## 9. 导出与导入需求

### 9.1 导出 export

支持导出以下数据：

- 应用配置
- 中央索引与单盘索引
- 会话历史（plan/rollback）

要求：

- ZIP 打包，包含 `manifest.json`
- 默认不包含敏感信息（如 API Key）
- 通过 `--include-secrets` 显式包含
- 支持按类别导出（`--only/--exclude`）

### 9.2 导入 import

要求：

- 支持 dry-run 预览
- 支持 merge 合并与 force 覆盖
- 支持导入前自动备份（`--backup-first`）
- 出错时给出冲突与失败摘要

---

## 10. 配置与环境需求

### 10.1 运行环境

- 平台：Linux
- 依赖：Rust、ffprobe（ffmpeg）、TMDB API Key
- 网络：需可访问 TMDB API；在受限网络环境可通过代理访问

### 10.2 AI 能力（可选，默认非必需）

✅ **核心原则（必须遵守）**
1. **AI 默认关闭** - 所有功能默认不依赖AI，用户必须显式开启
2. **本地解析优先** - 本地解析引擎是主流程，必须100%独立可用
3. **AI 后置增强** - AI仅在本地解析完成之后执行，作用为校验和补充信息
4. **永不替代** - 禁止将AI配置为替代本地解析的唯一路径

📌 **实现要求**
- AI 使用本地 Ollama 服务
- 支持配置 AI 开关、endpoint、model
- 设计上优先非 AI 基础流程，确保无 AI 环境可完整运行

### 10.3 配置默认值示例（建议）

```toml
[parser]
# 当前版本仅支持 local（纯 Rust 本地解析）
# 非 local 配置值必须视为配置错误并终止执行
engine = "local"
# 解析准确率优化目标：优先通过本地规则/模型提升
accuracy_mode = "max"

[ai]
# 关键默认值：AI 默认关闭
enabled = false
# 仅当 enabled=true 时生效
provider = "ollama"
endpoint = "http://localhost:11434"
model = "qwen2.5:7b"
# AI 在本地解析之后执行，作为增强/校验
mode = "post_check"
```

配置语义要求：

- 当 `ai.enabled=false`：只运行本地解析流程。
- 当 `ai.enabled=true`：执行顺序固定为“本地解析 -> AI 后置校验/补充 -> 最终结果”。
- 禁止将 AI 配置为替代本地解析的唯一路径。
- `parser.engine` 当前可选值仅 `local`；任何其他值必须报配置错误。

建议环境变量：

- `TMDB_API_KEY`（必需）
- `OLLAMA_BASE_URL`（默认 `http://localhost:11434`）
- `OLLAMA_MODEL`（默认 `qwen2.5:7b`）

---

## 11. 性能与并发需求

- ffprobe 可并发处理
- 海报下载可并发处理
- TMDB 请求需考虑限流与重试
- TV 场景支持季级缓存，减少重复查询
- 对已整理内容走快速路径，减少 AI 与搜索调用

---

## 12. 错误处理需求

- AI 解析失败：标记 unknown 并跳过
- TMDB 无匹配或低置信：标记 unknown 并跳过
- ffprobe 失败：可回退到文件名技术信息
- 文件操作失败：中断并保留 rollback 记录
- 索引/导入导出失败：保留可恢复现场并输出明确错误

---

## 13. 非目标与边界

- 不提供 GUI / Web 页面
- 不把 AI 作为唯一解析依赖
- 不要求自动修复低质量命名源文件，只要求安全跳过并可追踪

---

## 14. 需求验收清单（最小可交付）

- [ ] 可对 movies/tv_series 分别执行 plan/execute/rollback
- [ ] 输出命名符合规范并生成 NFO
- [ ] 支持 sample/subtitle/extras 归档
- [ ] 支持 verify 子命令
- [ ] 支持 sessions 会话查询能力（list/show）
- [ ] 满足幂等性（重复执行结果一致）
- [ ] 支持 central index + search（含离线盘状态）
- [ ] 支持 export/import（含 dry-run、merge、force、backup-first）
- [ ] AI 可选且默认可关闭；关闭时核心流程可完整工作