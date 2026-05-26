# lmforge 日志系统架构

## 核心设计原则

### 1. 分层日志架构

```
Console Output (UI)          File Logs (Truth)
┌──────────────────┐        ┌─────────────────────┐
│ Stage-oriented   │        │ Complete rebuildable │
│ Human-readable   │        │ Machine-readable     │
│ Minimal output   │        │ Full context         │
│ Color-coded      │        │ JSONL structured     │
└──────────────────┘        └─────────────────────┘
```

### 2. Console Output = UI

**目标：**
- 人眼快速扫描
- 显示当前 stage 状态
- 简洁不刷屏
- 不输出 subprocess 详情

**格式规范：**

```text
[workspace] preparing build workspace
[packages ] resolving desktop profile
[overlay  ] applying branding overlay
[image    ] generating squashfs
[release  ] writing SHA256SUMS
```

**Stage 命名（现代 infra 风格）：**

| 传统命名 | 新命名 | 说明 |
|---------|--------|------|
| bootstrap | workspace | 工作区准备 |
| chroot | runtime | 运行时环境 |
| rootfs | filesystem | 文件系统 |
| binary | image | 图像生成 |
| finalize | release | 发布阶段 |

**颜色编码：**

| 类型 | 颜色 | 示例 |
|------|------|------|
| Stage 名称 | 蓝色 | `[image    ]` |
| 正常文本 | 白色 | `generating squashfs` |
| WARN | 黄色 | `[packages ] WARN: firmware missing` |
| ERROR | 红色 | `[image    ] ERROR: xorriso failed` |
| SUCCESS | 绿色 | `[release  ] complete` |
| Runtime/子进程 | 暗灰 | 子进程输出 |

### 3. File Logs = Truth

**目标：**
- 完整重建构建过程
- Debug 和 CI 分析
- 失败复现
- 可重复性保证

**必须包含：**
- ✅ Timestamp (ISO 8601)
- ✅ Stage 信息
- ✅ Build ID
- ✅ Workspace 路径
- ✅ Process argv
- ✅ Exit code
- ✅ 完整 stdout/stderr
- ✅ Runtime lifecycle 事件
- ✅ Mount/unmount 操作
- ✅ 清理操作

## 目录结构

```
output/
└── build-20260527-a1b2c3d4/
    ├── logs/
    │   ├── build.log           # 主日志（文本格式）
    │   ├── build.jsonl         # 结构化日志（JSONL）
    │   └── stages/
    │       ├── workspace.log   # 工作区阶段日志
    │       ├── packages.log    # 包安装阶段日志
    │       ├── overlay.log     # 覆盖层阶段日志
    │       ├── image.log       # 图像生成阶段日志
    │       ├── metadata.log    # 元数据阶段日志
    │       └── release.log     # 发布阶段日志
    ├── artifacts/              # 构建产物
    ├── metadata/               # 构建元数据
    ├── temp/                   # 临时文件
    └── cache/                  # 缓存
```

## Build ID 系统

### 格式
```
build-{YYYYMMDD}-{8位UUID}
```

示例：
```
build-20260527-a1b2c3d4
build-20260528-e5f6g7h8
```

### 隔离目录

每个构建 ID 创建独立目录：
- `artifacts/` - ISO, rootfs, manifest 等
- `logs/` - 所有日志文件
- `metadata/` - BUILDINFO.json 等
- `temp/` - 临时文件
- `cache/` - 包缓存等

## 日志级别使用指南

### INFO - 正常流程
```text
[workspace] preparing build workspace
[image    ] generating squashfs
[release  ] complete
```

### WARN - 非致命问题
```text
[packages ] WARN: firmware package missing
[overlay  ] WARN: hook failed (non-fatal)
```

### ERROR - 阶段失败
```text
[image    ] ERROR: xorriso failed
[image    ] failed: grub-mkstandalone exited with status 1
```

### DEBUG - 详细信息（仅写入文件）
```text
2026-05-27T12:00:05Z DEBUG image:
exec: mksquashfs rootfs filesystem.squashfs -comp zstd

stdout:
Parallel mksquashfs: Using 16 processors

stderr:
warning: xattr unsupported
```

## Runtime 日志记录

所有运行时操作必须完整记录：

### 进程执行
```json
{
  "timestamp": "2026-05-27T12:00:03Z",
  "level": "info",
  "stage": "image",
  "message": "exec: mksquashfs rootfs filesystem.squashfs -comp zstd",
  "command": "mksquashfs",
  "args": ["rootfs", "filesystem.squashfs", "-comp", "zstd"],
  "working_dir": "/tmp/workspace",
  "exit_code": 0,
  "duration_ms": 82000,
  "stdout_len": 1024,
  "stderr_len": 256,
  "build_id": "build-20260527-a1b2c3d4"
}
```

### 挂载操作
```json
{
  "timestamp": "2026-05-27T12:01:00Z",
  "level": "info",
  "stage": "workspace",
  "message": "mount: /proc -> /workspace/rootfs/proc (proc)",
  "source": "/proc",
  "target": "/workspace/rootfs/proc",
  "fs_type": "proc",
  "build_id": "build-20260527-a1b2c3d4"
}
```

### Workspace 生命周期
```json
{
  "timestamp": "2026-05-27T12:00:00Z",
  "level": "info",
  "stage": "workspace",
  "action": "create",
  "path": "./workspace",
  "build_id": "build-20260527-a1b2c3d4"
}
```

## Structured Logging (JSONL)

每条日志一行 JSON：

```json
{"timestamp":"2026-05-27T12:00:03Z","level":"INFO","stage":"image","message":"generating squashfs","build_id":"build-20260527-a1b2c3d4"}
{"timestamp":"2026-05-27T12:00:05Z","level":"DEBUG","stage":"runtime","message":"exec: mksquashfs rootfs filesystem.squashfs -comp zstd","build_id":"build-20260527-a1b2c3d4"}
{"timestamp":"2026-05-27T12:01:22Z","level":"INFO","stage":"image","message":"squashfs generation completed","build_id":"build-20260527-a1b2c3d4"}
```

**用途：**
- CI/CD 解析
- Telemetry 收集
- Dashboard 展示
- Machine-readable analysis
- 故障排查自动化

## 使用示例

### 基本用法
```rust
use tracing::{info, warn, error};
use crate::telemetry::{stage_info!, stage_warn!, stage_error!};

// Stage-aware logging
stage_info!("workspace", "preparing build environment");
stage_warn!("packages", "firmware package missing");
stage_error!("image", "xorriso failed with exit code 1");

// Standard logging with target
info!(target: "lmforge_workspace", arch = "amd64", suite = "bookworm", "context initialized");
debug!(target: "lmforge_runtime", process = "debootstrap", args_count = 5, "executing");
```

### Runtime Logger
```rust
use crate::telemetry::runtime::RuntimeLogger;

let logger = RuntimeLogger::new("build-20260527-a1b2c3d4");

// 进程生命周期
logger.log_process_start("debootstrap", &["--arch=amd64", "bookworm"], Some(&PathBuf::from("/tmp")));
logger.log_process_complete("debootstrap", 0, Duration::from_secs(120), &stdout, &stderr);

// 文件系统操作
logger.log_mount(&PathBuf::from("/proc"), &PathBuf::from("/chroot/proc"), "proc");
logger.log_unmount(&PathBuf::from("/chroot/proc"));

// Workspace 生命周期
logger.log_workspace_create(&PathBuf::from("./workspace"));
logger.log_workspace_cleanup(&PathBuf::from("./workspace"));

// Stage 生命周期
logger.log_stage_start("workspace");
logger.log_stage_complete("workspace", Duration::from_secs(30));
logger.log_stage_error("image", "mksquashfs out of memory");
```

## 配置选项

### Console 输出控制
```toml
[telemetry]
enable_logging = true
log_level = "info"          # console 只显示 info 及以上
console_color = true         # 启用颜色输出
stage_width = 10             # stage 名称固定宽度
```

### 文件日志配置
```toml
[file_logs]
enabled = true
format = ["text", "jsonl"]  # 同时生成文本和 JSONL
level = "debug"              # 文件日志包含 debug
retention_days = 30
max_size_mb = 100
```

### Stage 日志隔离
```toml
[stage_logs]
enabled = true
per_stage_files = true      # 每个 stage 独立日志文件
include_subprocess = true   # 包含完整的 stdout/stderr
structured_output = true    # JSON 格式元数据
```

## 最佳实践

### ✅ 推荐做法

1. **始终使用 stage-aware logging**
   ```rust
   stage_info!("workspace", "creating directories"); // ✅
   info!("creating directories");                    // ❌ 缺少 stage 上下文
   ```

2. **Runtime 操作使用 RuntimeLogger**
   ```rust
   logger.log_process_start("apt-get", &["update"], None); // ✅
   debug!("running apt-get update");                       // ❌ 不够详细
   ```

3. **Error 包含上下文**
   ```rust
   stage_error!("image", 
       command = "xorriso",
       exit_code = 1,
       stderr = %output.stderr,
       "image generation failed"  // ✅ 完整上下文
   );
   ```

4. **Timing 仅作为辅助信息**
   ```rust
   stage_info!("workspace", "[02:34] completed");  // ✅ 可选 timing
   // 不要像 benchmark 工具那样输出大量 metrics
   ```

### ❌ 避免做法

1. **禁止 println! spam**
   ```rust
   println!("Building...");           // ❌
   println!("Step 1 of 10");          // ❌
   println!("Progress: 45%");         // ❌
   ```

2. **禁止 Web backend 风格**
   ```json
   {"severity": "ERROR", "service": "lmforge", ...}  // ❌ 过度工程化
   ```

3. **禁止游戏插件风格**
   ```text
   [*Err] Something went wrong!  // ❌
   [!!!] CRITICAL FAILURE!       // ❌
   =============================  // ❌
   ```

4. **禁止刷屏输出**
   ```text
   Installing package 1 of 234...
   Installing package 2 of 234...
   Installing package 3 of 234...
   // ... 231 lines later
   ```

## 与其他工具对比

### Cargo 风格
```text
 Compiling lmforge v0.1.0
 Finished dev [unoptimized + debuginfo] target(s) in 42.12s
```
✅ 我们采用类似简洁风格

### Nix 风格
```text
building '/nix/store/xxx.drv'...
these 5 paths will be fetched (123.45 MiB)...
copying path '/nix/store/yyy' from 'cache'...
```
✅ Stage-oriented + 进度信息

### Bazel 风格
```text
INFO: Analyzed 23 targets (0 packages loaded).
INFO: Found 23 targets...
INFO: Elapsed time: 123.456s, Critical Path: 89.12s
```
✅ 结构化 + Timing + Summary

### Docker 风格
```text
Step 1/5 : FROM debian:bookworm
 ---> abcdef123456
Step 2/5 : RUN apt-get update
 ---> Using cache
 ---> def456789012
```
✅ Layered stages + Caching info

## 扩展性设计

### 未来支持

1. **Distributed builds**
   - Build ID 支持节点标识
   - 日志聚合到中央服务器

2. **Telemetry dashboard**
   - JSONL 直接导入数据库
   - 实时构建状态展示

3. **CI/CD integration**
   - 自动解析 JSONL 生成报告
   - 失败分析自动化

4. **Build farm**
   - 多机器并行构建
   - 统一日志收集和分析

## 性能考虑

### 异步写入
- 文件日志异步 flush
- 不阻塞主构建流程

### 日志轮转
- 单文件最大 100MB
- 自动压缩历史日志
- 可配置保留期

### 内存优化
- Console buffer 限制
- 文件日志批量写入
- 避免频繁 I/O

---

## 总结

lmforge 的日志系统专为**工业级发行版构建平台**设计：

✅ **分层架构** - Console UI vs File Truth  
✅ **Stage-oriented** - 构建流水线优先  
✅ **完整可重建** - 文件日志包含所有细节  
✅ **Structured** - JSONL 支持 machine-readable  
✅ **Runtime-first** - 所有系统调用完整记录  
✅ **Build isolation** - 每次构建独立标识和目录  
✅ **Production-ready** - 参考 Cargo/Nix/Bazel/Docker 风格  

这不是一个简单的 CLI 工具日志系统，而是一个**构建基础设施平台的可观测性核心**。
