# 快速开始指南

**Media Organizer** 是使用 Rust 编写的高性能媒体整理工具，基于本地 AI 自动识别并标准化您的电影和电视剧收藏。

---

## ✅ 系统要求

| 组件 | 最低要求 | 推荐配置 |
|------|----------|----------|
| 操作系统 | Linux x86_64 | 最新 LTS 发行版 |
| 内存 | 8 GB | 16 GB |
| CPU | 4 核心 | 8 核心以上 |
| GPU | 可选 | NVIDIA/AMD 显卡 (加速 AI 推理) |

**必需依赖:**
- `ffprobe` (ffmpeg 组件) - 视频元数据提取
- `TMDB API Key` - 影视元数据

**可选依赖 (高级功能):**
- `Ollama` - 本地 AI 推理引擎
- `qwen2.5:7b` - 文件名解析模型

> ℹ️ **重要**: Ollama AI 是可选高级功能，默认不启用。即使不安装 AI，工具所有核心功能 100% 可用。

---

## 🚀 5分钟快速上手

### 1. 环境准备

```bash
# ✅ 必需: 安装 ffmpeg
sudo apt update && sudo apt install ffmpeg

# ✅ 必需: 申请 TMDB API Key
# 访问: https://www.themoviedb.org/settings/api 注册获取

# ⚠️ 可选高级功能: 安装 Ollama AI (非必需)
# 只有当你需要使用 AI 智能解析时才需要安装
# curl -fsSL https://ollama.ai/install.sh | sh
# ollama pull qwen2.5:7b
```

### 2. 配置方式

支持两种配置方式 (二选一即可):

#### ✅ 方式1: 配置文件 (推荐)

```bash
# 创建配置目录
mkdir -p ~/.config/media_organizer

# 编辑配置文件 ~/.config/media_organizer/config.toml
```

```toml
[tmdb]
api_key = "你的 TMDB API 密钥"
language = "zh-CN"

[ollama]
host = "localhost"
port = 11434
model = "qwen2.5:7b"
timeout = 60
```

#### ⚠️ 方式2: 环境变量 (临时使用)

```bash
# 临时设置，仅对当前终端有效
export TMDB_API_KEY="你的 TMDB API 密钥"
```

> 💡 **优先级**: 环境变量 > 配置文件 > 默认值
> 所有参数都有合理的默认值，TMDB API Key 是唯一必须配置的项。
> 默认配置下不需要设置 OLLAMA 相关参数。

### 3. 安装程序

```bash
# 从源码编译
git clone https://github.com/jpixy/media_organizer.git
cd media_organizer/media_organizer_rs
cargo build --release

# 安装到系统
sudo cp target/release/media-organizer /usr/local/bin/

# 验证安装
media-organizer --version
```

### 4. 预检查

```bash
# 运行前置检查，确认所有依赖正常
media-organizer plan movies /tmp --dry-run
```

✅ 成功输出:
```
[OK] ffprobe: installed
[OK] Ollama: running (http://localhost:11434)
[OK] TMDB API: connected
[OK] 所有检查通过
```

---

## 🎬 第一个整理任务

### 整理电影

```bash
# ✅ 最简用法: 自动在源目录旁创建 `未整理电影_organized` 作为目标
media-organizer plan movies /下载/未整理电影

# 💡 指定目标目录 (可选)
media-organizer plan movies /下载/未整理电影 -t /媒体库/电影

# 检查生成的计划
ls -lh /下载/未整理电影_organized/plan_*.json

# 确认无误后执行
media-organizer execute /下载/未整理电影_organized/plan_*.json

# 如有问题可随时回滚
media-organizer rollback /下载/未整理电影_organized/rollback_*.json
```

### 整理电视剧

```bash
# ✅ 最简用法
media-organizer plan tv_series /下载/未整理剧集

# 💡 指定目标目录 (可选)
media-organizer plan tv_series /下载/未整理剧集 -t /媒体库/电视剧

media-organizer execute /下载/未整理剧集_organized/plan_*.json
```

---

## 📚 建立媒体索引

```bash
# 索引电影目录
media-organizer index scan /媒体库/电影 --media-type movies --disk-label 主硬盘

# 索引电视剧目录
media-organizer index scan /媒体库/电视剧 --media-type tv_series --disk-label 主硬盘

# 查看收藏统计
media-organizer index stats
```

---

## 🔍 搜索媒体

```bash
# 按标题搜索
media-organizer search --title "盗梦空间"

# 按演员搜索
media-organizer search --actor "莱昂纳多·迪卡普里奥"

# 按年份范围搜索
media-organizer search --year 2020-2025

# 组合搜索
media-organizer search --genre 科幻 --year 2010-2020
```

---

## ⚡ 常用命令速查表

| 命令 | 描述 |
|------|------|
| `plan movies <源> -t <目标>` | 生成电影整理计划 |
| `plan tv_series <源> -t <目标>` | 生成电视剧整理计划 |
| `execute <plan.json>` | 执行整理计划 |
| `rollback <rollback.json>` | 回滚操作 |
| `index scan <路径>` | 建立媒体索引 |
| `index stats` | 查看收藏统计 |
| `search --title <关键词>` | 搜索媒体 |
| `export --auto-name` | 完整备份所有数据 |
| `import <备份文件>` | 恢复备份 |

---

## 📖 下一步

- 📘 [完整用户手册](USER-MANUAL.md) - 所有命令详解
- 📙 [中央索引系统](04-central-index.md) - 跨硬盘搜索工作原理
- 📗 [备份与恢复](05-export-import.md) - 数据安全
- 📕 [GPU 加速配置](06-gpu-setup.md) - 优化 AI 推理速度