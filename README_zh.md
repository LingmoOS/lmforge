# lmforge

LingmoOS 构建编译工具（lmforge）- Linux 发行版构建工具。

## 概述

lmforge 用于编译 Linux 发行版镜像。它管理构建工作空间、配置、运行时执行和产物收集。

项目当前面向基于 Debian 的发行版。实现上使用 live-build 作为后端引擎。lmforge 负责构建协调、工作空间生命周期管理、覆盖层管理和清理恢复。

## 架构

```
lmforge (构建层)
  ├── WorkspaceManager  (构建隔离)
  ├── LiveBuildEngine   (live-build 封装)
  ├── OverlayManager    (文件系统/品牌/钩子)
  ├── ArtifactManager   (ISO + 校验和 + 清单)
  └── CleanupRecovery   (失败恢复 + 过期清理)

运行时层
  └── ProcessRunner     (进程执行，stdout/stderr 捕获)

后端
  └── live-build        (lb config / lb build / lb clean)
      └── debootstrap / mmdebstrap
```

构建协调使用同步接口。运行时内部在需要时使用 tokio 执行异步操作。

## 功能

- **CLI 接口**：支持 build、config、package 等操作
- **工作空间隔离**：每次构建使用独立目录
- **配置系统**：支持预设和用户自定义覆盖
- **live-build 集成**：管理 config、build、clean 完整生命周期
- **覆盖层支持**：文件系统覆盖、品牌定制、钩子注入、软件包列表
- **产物收集**：ISO 收集、SHA256 校验和生成、构建清单生成
- **清理与恢复**：锁文件管理、中断构建检测、过期工作空间清除
- **结构化日志**：按阶段输出日志

## 构建流程

```
1. 加载配置（预设 → 覆盖 → 用户配置）
2. 初始化工作空间（output/build-<时间戳>-<构建ID>/）
3. 设置清理恢复机制（锁文件、PID 追踪）
4. 执行流水线阶段：
   - workspace: 准备构建目录
   - bootstrap: 通过 debootstrap/mmdebstrap 构建基础系统
   - packages: 安装软件包到 rootfs
   - overlay: 应用文件系统/品牌/定制内容
   - image: 运行 live-build 生成 ISO
   - metadata: 生成清单和校验和
   - release: 收集产物到输出目录
5. 标记构建完成或触发失败恢复
6. 清理临时文件
```

输出目录结构：
```
output/build-20260529-abcdef1234/
├── config/           # live-build 配置
├── cache/            # 缓存下载
├── artifacts/        # 最终 ISO + 校验和 + 清单
│   ├── lingmo-live.iso
│   ├── SHA256SUMS
│   └── build-manifest.json
├── logs/
│   └── stages/       # 各阶段日志文件
├── runtime/          # 运行时状态
├── temp/             # 临时文件
├── rootfs/           # 根文件系统
└── overlay/          # 自定义覆盖层
    ├── filesystem/
    ├── branding/
    ├── packages.list
    └── hooks/
```

## 安装

### 环境要求

- Rust 工具链（1.70+）
- 构建镜像需要：live-build、debootstrap 或 mmdebstrap

### 从源码编译

```bash
git clone <仓库地址>
cd lmforge
cargo build --release
```

编译产物位置：`target/release/lmforge`

## 使用方法

### 构建 ISO 镜像

```bash
# 使用默认配置构建
lmforge build iso official

# 详细输出，显示各阶段信息
lmforge build iso official -v

# 空运行模式，不实际执行命令
lmforge build iso official --dry-run

# 指定输出目录
lmforge build iso official --output ./my-builds

# 覆盖架构和套件
lmforge build iso official --arch amd64 --suite bookworm
```

### 查看当前配置

```bash
# 显示解析后的配置
lmforge config --show

# 使用指定配置文件
lmforge config --show --config ./my-config.toml
```

### 构建选项

```
选项:
  -v, --verbose       启用详细输出
  --config <PATH>     配置文件路径
  --output <PATH>     输出目录
  --workspace <PATH>  工作空间基础目录
  --arch <ARCH>       目标架构 (amd64, arm64)
  --suite <SUITE>     Debian 套件 (bookworm, trixie, sid)
```

## 配置

配置按以下顺序加载（后加载的覆盖先前的）：

1. **内置预设**：已知发行版的默认设置
2. **覆盖文件**：项目特定配置
3. **用户配置**：`--config` 参数指定的路径或默认路径

示例配置（`lingmo-official.toml`）：
```toml
[project]
name = "Lingmo Linux"
version = "1.0"

[platform]
name = "debian"
suite = "bookworm"
arch = "amd64"
components = ["main", "contrib", "non-free"]

[build]
workspace_dir = "./output"
clean_before_build = false

[output]
format = "iso"
compression = "zstd"
```

### 覆盖层

将定制文件放置在 overlay 目录下：

```
overlay/
├── filesystem/         # 复制到 rootfs 的文件
│   └── etc/
│       └── custom.conf
├── branding/           # 发行版品牌信息
│   └── etc/
│       ├── issue
│       └── os-release
├── packages.list       # 额外软件包（每行一个）
└── hooks/              # live-build 钩子
    ├── 01-custom.chroot
    └── 02-postinstall.chroot
```

构建时会自动同步到 live-build 配置目录。

## 项目状态

**早期开发阶段**

- API 不稳定，版本间可能变化
- 当前聚焦于 Debian/bookworm + live-build 后端
- 运行时生命周期管理仍在完善中
- 测试覆盖率有限
- 文档不完整

这不是可用于生产环境的软件。仅用于开发和测试。

## 开发

### 运行测试

```bash
cargo test
```

### 启用调试输出构建

```bash
RUST_LOG=debug cargo run -- build iso official -v
```

## 许可证

GPL-2.0 许可证。详见 [LICENSE](LICENSE) 文件。
