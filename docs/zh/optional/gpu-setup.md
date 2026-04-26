# 06 - Ollama GPU 配置指南

本文档记录了在 Fedora Linux 上配置 Ollama 使用 NVIDIA GPU 的完整过程。

---

## 1. 问题描述

### 症状

1. Ollama 启动时显示 `entering low vram mode` 和 `total vram="0 B"`
2. 推理速度极慢，经常超时（>180秒）
3. 日志显示 `initial_count=0`，表示没有检测到任何 GPU
4. `nvidia-smi` 命令不存在或显示 `nouveau` 驱动

### 日志示例

```
time=2025-12-29T16:48:22.901+08:00 level=INFO source=types.go:60 msg="inference compute" id=cpu library=cpu
time=2025-12-29T16:48:22.901+08:00 level=INFO source=routes.go:1648 msg="entering low vram mode" "total vram"="0 B"
```

---

## 2. 环境信息

| 项目 | 版本/信息 |
|------|----------|
| **操作系统** | Fedora 42 (Linux 6.17.13) |
| **GPU** | NVIDIA RTX 3500 Ada Generation Laptop GPU |
| **显存** | 12GB |
| **Ollama 版本** | 0.13.5 |
| **CUDA 版本** | 13.0 |
| **驱动版本** | 580.119.02 |

---

## 3. 诊断步骤

### 步骤 1: 检查 GPU 硬件

```bash
lspci | grep -i nvidia
```

预期输出：
```
01:00.0 3D controller: NVIDIA Corporation AD106M [GeForce RTX 3500 Ada Generation Laptop GPU]
```

### 步骤 2: 检查当前驱动

```bash
lsmod | grep -E "nvidia|nouveau"
```

- 如果显示 `nouveau`：使用的是开源驱动，需要安装 NVIDIA 专有驱动
- 如果显示 `nvidia`：专有驱动已加载

### 步骤 3: 检查 nvidia-smi

```bash
nvidia-smi
```

如果命令不存在或报错，说明 NVIDIA 驱动未正确安装。

### 步骤 4: 检查 NVML 库

```bash
ldconfig -p | grep nvidia-ml
```

如果没有输出，需要安装 CUDA 库包。

### 步骤 5: 检查 Ollama CUDA 库

```bash
ls -la /usr/lib/ollama/cuda_v12/
ls -la /usr/lib/ollama/cuda_v13/
```

---

## 4. 解决方案

### 问题 1: NVIDIA 驱动未安装

**症状**: `nvidia-smi` 不存在，`lsmod` 显示 `nouveau`

**解决方案**:

```bash
# 1. 启用 RPM Fusion 仓库
sudo dnf install \
  https://download1.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm \
  https://download1.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-$(rpm -E %fedora).noarch.rpm

# 2. 安装 NVIDIA 驱动和 CUDA 库
sudo dnf install akmod-nvidia xorg-x11-drv-nvidia-cuda xorg-x11-drv-nvidia-cuda-libs

# 3. 等待 akmods 编译内核模块（约 5-10 分钟）
sudo akmods --force

# 4. 重启系统
sudo reboot
```

### 问题 2: libnvidia-ml 不在 ldconfig 缓存中

**解决方案**:

```bash
sudo ldconfig
ldconfig -p | grep nvidia-ml
```

### 问题 3: Ollama CUDA 库缺失

**解决方案**:

```bash
# 方法 1: 从 GitHub 下载并安装
wget -O /tmp/ollama.tgz \
  "https://github.com/ollama/ollama/releases/download/v0.13.5/ollama-linux-amd64.tgz"
sudo tar -C /usr -xzf /tmp/ollama.tgz

# 方法 2: 使用官方安装脚本重新安装
curl -fsSL https://ollama.com/install.sh | sh
```

### 问题 4: Ollama 使用错误的库路径

**解决方案**:

```bash
# 创建符号链接
sudo rm -rf /usr/local/lib/ollama
sudo ln -sf /usr/lib/ollama /usr/local/lib/ollama

# 验证
ls -la /usr/local/lib/ollama/cuda_v12/
```

---

## 5. 验证步骤

### 步骤 1: 验证驱动加载

```bash
nvidia-smi
```

预期输出：
```
+-----------------------------------------------------------------------------------------+
| NVIDIA-SMI 580.119.02             Driver Version: 580.119.02     CUDA Version: 13.0     |
+-----------------------------------------+------------------------+----------------------+
| GPU  Name                 Persistence-M | Bus-Id          Disp.A | Volatile Uncorr. ECC |
|   0  NVIDIA RTX 3500 Ada Gene...    Off |   00000000:01:00.0 Off |                  Off |
+-----------------------------------------+------------------------+----------------------+
```

### 步骤 2: 验证 Ollama GPU 检测

```bash
ollama serve 2>&1 | head -30
```

预期日志（GPU 检测成功）：
```
msg="inference compute" id=GPU-xxx library=CUDA compute=8.9 name=CUDA0 
description="NVIDIA RTX 3500 Ada Generation Laptop GPU" 
total="12.0 GiB" available="11.6 GiB"
```

### 步骤 3: 验证 GPU 推理

```bash
# 发送测试请求
curl -s http://localhost:11434/api/generate \
  -d '{"model":"qwen2.5:7b","prompt":"Hello","stream":false}' &

# 检查 GPU 使用
sleep 5
nvidia-smi
```

预期输出（GPU 进程存在）：
```
| Processes:                                                                              |
|    0   N/A  N/A   xxxxx      C   /usr/local/bin/ollama                  4866MiB |
```

### 步骤 4: 性能对比

| 模式 | 首次加载时间 | 推理时间 (简单提示) |
|------|-------------|-------------------|
| CPU | >180秒 (超时) | >60秒 |
| GPU | ~30秒 | ~5秒 |

---

## 6. 常见问题

### Q1: 重启后 GPU 又不工作了

检查内核模块是否加载：
```bash
lsmod | grep nvidia
```
如果没有，可能是内核更新后模块需要重新编译：
```bash
sudo akmods --force
sudo reboot
```

### Q2: Ollama 显示 "entering low vram mode"

这是正常的，当 GPU 显存 < 20GB 时会显示此消息。只要日志中有 `library=CUDA`，就表示 GPU 正在被使用。

### Q3: 如何查看 Ollama 使用的是哪个 CUDA 版本？

查看日志中的 `libdirs` 字段：
```
libdirs=ollama,cuda_v13  # 使用 CUDA 13
libdirs=ollama,cuda_v12  # 使用 CUDA 12
```

### Q4: [GIN] 日志是什么意思？

Ollama 使用 Go 语言的 Gin 框架作为 HTTP 服务器。`[GIN]` 日志是 HTTP 请求的访问日志：
```
[GIN] 2025/12/29 - 17:36:45 | 200 | 40.227045034s | 127.0.0.1 | POST "/api/generate"
```
- `200`: HTTP 状态码
- `40.227045034s`: 请求处理时间
- `POST "/api/generate"`: API 端点

---

## 7. 快速诊断脚本

```bash
#!/bin/bash
echo "=== GPU Hardware ==="
lspci | grep -i nvidia

echo ""
echo "=== Driver Module ==="
lsmod | grep -E "nvidia|nouveau"

echo ""
echo "=== NVIDIA Driver ==="
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv 2>/dev/null || echo "nvidia-smi unavailable"

echo ""
echo "=== NVML Library ==="
ldconfig -p | grep nvidia-ml

echo ""
echo "=== Ollama Libraries ==="
ls -la /usr/lib/ollama/cuda_v* 2>/dev/null || ls -la /usr/local/lib/ollama/cuda_v* 2>/dev/null

echo ""
echo "=== Ollama Status ==="
pgrep -x ollama && echo "Ollama is running" || echo "Ollama is not running"
```

---

## 8. 参考链接

- [Ollama 官方文档](https://ollama.com/docs)
- [RPM Fusion NVIDIA 驱动指南](https://rpmfusion.org/Howto/NVIDIA)
- [NVIDIA CUDA Toolkit 文档](https://developer.nvidia.com/cuda-toolkit)

