# lmforge Debian 测试指南

## 📋 系统要求

### 最低配置
- **操作系统**: Debian 12 (Bookworm) 或更新版本
- **架构**: amd64 (x86_64)
- **内存**: 4GB RAM (推荐 8GB+)
- **磁盘空间**: 20GB 可用空间 (用于 workspace 和输出)
- **网络**: 需要互联网连接（下载包和依赖）

### 推荐配置
- **内存**: 8-16 GB RAM
- **CPU**: 4 核心以上
- **磁盘**: SSD, 50GB+ 空间
- **Debian 版本**: Bookworm/Sid (开发测试)

---

## 🔧 第一步：安装系统依赖

### 1.1 更新系统包管理器
```bash
sudo apt update && sudo apt upgrade -y
```

### 1.2 安装构建工具链
```bash
sudo apt install -y \
    build-essential \
    pkg-config \
    cmake \
    git \
    curl \
    wget
```

### 1.3 安装 Rust 工具链
```bash
# 安装 rustup（如果尚未安装）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 按照提示选择默认选项（按 Enter）

# 加载 Rust 环境
source $HOME/.cargo/env

# 验证安装
rustc --version
cargo --version
```

**预期输出:**
```
rustc 1.75.0 (82e1608df 2023-12-21)
cargo 1.75.0 (1d8b796f2 2023-11-27)
```

### 1.4 安装 lmforge 运行时依赖
```bash
sudo apt install -y \
    debootstrap \
    live-build \
    squashfs-tools \
    xorriso \
    isolinux \
    syslinux-common \
    grub-pc-bin \
    grub-efi-amd64-bin \
    dosfstools \
    mtools \
    fdisk \
    parted \
    gdisk
```

### 1.5 安装可选依赖（用于完整功能）
```bash
# 桌面环境构建支持
sudo apt install -y \
    gnome-core \
    calamares \
    live-boot \
    live-config \
    live-config-systemd

# 开发调试工具
sudo apt install -y \
    strace \
    ltrace \
    gdb \
    valgrind \
    file \
    tree \
    htop
```

---

## 💿 第二步：获取并编译 lmforge

### 2.1 克隆项目（如果从 Git）
```bash
cd /opt
sudo mkdir -p lmforge-dev
sudo chown $USER:$USER lmforge-dev
cd lmforge-dev

git clone <your-repo-url> lmforge
cd lmforge
```

### 2.2 或者使用现有代码
```bash
# 假设代码在 d:\Projects\rust\lmforge (Windows)
# 在 WSL 或 Linux 中访问：
cd /mnt/d/Projects/rust/lmforge
```

### 2.3 编译项目
```bash
# 开发模式编译（快速）
cargo build

# 或者发布模式编译（优化性能）
cargo build --release

# 验证编译成功
./target/debug/lmforge --version
# 或
./target/release/lmforge --version
```

**预期输出:**
```
lmforge 0.1.0
```

### 2.4 解决常见编译问题

#### 问题 1: 缺少 OpenSSL 库
```bash
sudo apt install -y libssl-dev pkg-config
```

#### 问题 2: 缺少其他库
```bash
# 如果遇到链接错误
sudo apt install -y libclang-dev clang
export LIBCLANG_PATH=/usr/lib/llvm-14/lib
```

---

## 🧪 第三步：基本功能测试

### 3.1 测试 CLI 帮助
```bash
./target/debug/lmforge --help
```

**预期输出示例:**
```
Industrial-grade Linux distribution build platform

Usage: lmforge [OPTIONS] <COMMAND>

Commands:
  build      Build ISO images and distributions
  package    Package management operations
  repo       Repository management
  config     Show configuration
  help       Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose           Verbose output
  -c, --config <CONFIG>   Config file path
  -o, --output <OUTPUT>   Output directory
  -w, --workspace <WORKSPACE>
                          Workspace directory
  --arch <ARCH>           Target architecture
  --suite <SUITE>         Target suite
  -h, --help              Print help
  -V, --version           Print version
```

### 3.2 测试子命令帮助
```bash
./target/debug/lmforge build --help
./target/debug/lmforge config --help
./target/debug/lmforge package --help
```

### 3.3 测试配置生成
```bash
# 生成默认配置文件
./target/debug/lmforge config --generate

# 查看生成的配置
cat lmforge.toml
```

### 3.4 测试配置验证
```bash
./target/debug/lmforge config --show --validate
```

---

## 🚀 第四步：最小化构建测试

### 4.1 准备测试目录
```bash
mkdir -p ~/lmforge-test
cd ~/lmforge-test
```

### 4.2 使用 minimal preset 进行 rootfs 构建
```bash
# 最小化 rootfs 构建测试
../lmforge/target/debug/lmforge build rootfs i386 minimal \
    --output ./output \
    --workspace ./workspace \
    --verbose
```

**预期 Console 输出:**
```
lmforge v0.1.0 starting
build_id=build-20260527-a1b2c3d4 version=0.1.0

[workspace] preparing build workspace
[workspace] bootstrapping Debian bookworm (i386)
[packages ] no additional packages to install
[release  ] complete

Build completed:
  build_id: build-20260527-a1b2c3d4
  duration: 2m 15s
  stages: 3/3
  artifacts: 1
    - rootfs.tar.zst (156 MB)

Output: ./output/build-20260527-a1b2c3d4/artifacts/
Logs:   ./output/build-20260527-a1b2c3d4/logs/
```

### 4.3 检查构建产物
```bash
# 查看产物目录
ls -lh ./output/build-*/artifacts/

# 查看 rootfs 大小
du -sh ./output/build-*/artifacts/rootfs.tar.zst

# 检查日志文件
ls -lh ./output/build-*/logs/
ls -lh ./output/build-*/logs/stages/
```

### 4.4 查看详细日志
```bash
# 主日志
cat ./output/build-*/logs/build.log

# JSONL 结构化日志（前10行）
head -n 10 ./output/build-*/logs/build.jsonl | jq .

# 特定阶段日志
cat ./output/build-*/logs/stages/workspace.log
```

---

## 🖥️ 第五步：ISO 构建测试（需要更多资源）

### 5.1 准备 ISO 构建
```bash
mkdir -p ~/lmforge-iso-test
cd ~/lmforge-iso-test
```

### 5.2 使用 official preset 构建 ISO
```bash
../lmforge/target/debug/lmforge build iso amd64 official \
    --output ./output \
    --workspace ./workspace \
    --clean
```

**注意:** 这可能需要 10-30 分钟，取决于您的系统性能和网络速度。

**预期输出包含:**
```
[workspace] preparing build workspace
[packages ] installing base packages
[overlay  ] applying filesystem overlay
[image    ] generating squashfs
[image    ] generating EFI bootloader
[image    ] generating ISO image
[metadata] generating MANIFEST
[metadata] writing SHA256SUMS
[release  ] complete

Build completed:
  artifacts: 3+
    - lingmo-live-amd64.iso (~2GB)
    - SHA256SUMS
    - BUILDINFO.json
```

### 5.3 验证 ISO 文件
```bash
# 检查 ISO 是否存在
ls -lh ./output/build-*/artifacts/*.iso

# 验证 ISO 格式
file ./output/build-*/artifacts/*.iso

# 预期输出: ... ISO 9660 CD-ROM filesystem data ...

# 检查校验和
sha256sum -c ./output/build-*/artifacts/SHA256SUMS
```

### 5.4 测试带特性的构建
```bash
# 带 desktop + live + installer 的完整构建
../lmforge/target/debug/lmforge build iso amd64 desktop \
    --desktop \
    --live \
    --installer \
    --output ./output-desktop \
    --workspace ./workspace-desktop
```

⚠️ **警告:** 完整桌面版构建可能需要 30-60 分钟和 8GB+ 内存。

---

## 🔍 第六步：日志系统测试

### 6.1 测试 Console 输出格式
```bash
# 正常模式
../lmforge/target/debug/lmforge build rootfs i386 minimal 2>&1 | head -n 20

# Debug 模式（查看更详细信息）
RUST_LOG=debug ../lmforge/target/debug/lmforge build rootfs i386 minimal 2>&1 | head -n 50
```

### 6.2 测试 File Log 系统
```bash
# 执行一次构建后检查日志结构
find ./output/build-* -name "*.log" -o -name "*.jsonl" | sort

# 查看 Stage 日志隔离
echo "=== Workspace Stage ==="
tail -n 5 ./output/build-*/logs/stages/workspace.log

echo "=== Packages Stage ==="
tail -n 5 ./output/build-*/logs/stages/packages.log

# 解析 JSONL 日志
jq 'select(.level == "INFO") | "\(.timestamp) [\(.stage)] \(.message)"' \
    ./output/build-*/logs/build.jsonl | head -n 20
```

### 6.3 测试 Runtime 日志记录
```bash
# 搜索进程执行日志
grep "exec:" ./output/build-*/logs/build.log | head -n 10

# 搜索挂载操作日志
grep "mount:" ./output/build-*/logs/build.log | head -n 10

# 在 JSONL 中查找错误
jq 'select(.level == "ERROR")' ./output/build-*/logs/build.jsonl
```

### 6.4 测试 Build ID 隔离
```bash
# 连续执行两次构建
../lmforge/target/debug/lmforge build rootfs i386 minimal --output ./run1
../lmforge/target/debug/lmforge build rootfs i386 minimal --output ./run2

# 检查是否生成不同的 build ID
ls -d ./run1/build-* ./run2/build-*

# 验证目录隔离
echo "Run 1:"
ls ./run1/build-*/logs/

echo "Run 2:"
ls ./run2/build-*/logs/
```

---

## 🛠️ 第七步：故障排查

### 7.1 常见问题及解决方案

#### 问题 1: 权限不足
**错误:** `Permission denied` when running debootstrap

**解决方案:**
```bash
# 方法 1: 使用 sudo（不推荐，但快速测试）
sudo ../lmforge/target/debug/lmforge build rootfs i386 minimal

# 方法 2: 将用户添加到相关组（推荐）
sudo usermod -aG disk $USER
sudo usermod -aG kvm $USER
# 注销并重新登录生效
```

#### 问题 2: 内存不足
**错误:** `mksquashfs: out of memory`

**解决方案:**
```bash
# 检查可用内存
free -h

# 增加 swap 空间
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile

# 验证
free -h
```

#### 问题 3: 磁盘空间不足
**错误:** `No space left on device`

**解决方案:**
```bash
# 检查磁盘空间
df -h

# 清理旧的构建产物
rm -rf ./output/*
rm -rf ./.cache/*

# 或者清理 apt 缓存
sudo apt clean
```

#### 问题 4: 网络连接问题
**错误:** `Failed to fetch ... Connection timed out`

**解决方案:**
```bash
# 检查网络连接
ping deb.debian.org

# 配置代理（如果需要）
export http_proxy=http://proxy:port
export https_proxy=http://proxy:port

# 或者使用国内镜像（中国用户）
# 编辑 presets/official.toml，修改 mirror 为：
# mirror = "https://mirrors.tuna.tsinghua.edu.cn/debian"
```

#### 问题 5: debootstrap 未找到
**错误:** `debootstrap not found`

**解决方案:**
```bash
sudo apt install -y debootstrap

# 验证
which debootstrap
debootstrap --version
```

### 7.2 调试技巧

#### 启用 Debug 模式
```bash
RUST_LOG=debug ../lmforge/target/debug/lmforge build rootfs i386 minimal
```

#### 查看详细 subprocess 输出
```bash
# 文件日志中包含完整的 stdout/stderr
cat ./output/build-*/logs/stages/image.log | grep -A 50 "stdout:"
```

#### 使用 strace 跟踪系统调用
```bash
strace -f -e trace=network,process ../lmforge/target/debug/lmforge build rootfs i386 minimal 2>&1 | head -n 100
```

---

## 📊 第八步：性能基准测试

### 8.1 记录构建时间
```bash
# 使用 time 命令测量
time ../lmforge/target/debug/lmforge build rootfs i386 minimal

# 输出将包含:
# real    2m15.123s
# user    0m45.678s
# sys     0m12.345s
```

### 8.2 分析各阶段耗时
```bash
# 从 JSONL 提取 timing 数据
jq -r '
  select(.duration_ms != null) |
  "\(.stage): \(.duration_ms / 1000)s"
' ./output/build-*/logs/build.jsonl | sort -t: -k2 -rn
```

### 8.3 监控系统资源使用
```bash
# 终端 1: 运行构建
../lmforge/target/debug/lmforge build iso amd64 official &

# 终端 2: 监控资源
watch -n 1 '
  echo "=== CPU ===" 
  top -bn1 | head -n 5
  
  echo -e "\n=== Memory ==="
  free -h
  
  echo -e "\n=== Disk I/O ==="
  iostat -x 1 1
  
  echo -e "\n=== lmforge Process ==="
  ps aux | grep lmforge | grep -v grep
'
```

---

## ✅ 第九步：功能验证清单

完成以下测试项以验证 lmforge 功能完整性：

### 基础功能
- [ ] CLI 帮助信息正常显示
- [ ] 子命令帮助正常工作
- [ ] 配置文件生成成功
- [ ] 配置验证通过
- [ ] 版本号正确显示

### 构建功能
- [ ] Minimal rootfs 构建成功
- [ ] Official ISO 构建成功（如果资源允许）
- [ ] 产物文件存在且大小合理
- [ ] 校验和验证通过
- [ ] Manifest 文件正确生成

### 日志系统
- [ ] Console 输出格式符合规范（Stage-oriented）
- [ ] 颜色编码正确（蓝/黄/红/绿）
- [ ] build.log 文件存在且内容完整
- [ ] build.jsonl 文件存在且为有效 JSON
- [ ] 各 stage 独立日志文件存在
- [ ] Runtime 操作（进程/挂载）被完整记录
- [ ] Build ID 正确生成且唯一
- [ ] 目录结构符合预期

### 错误处理
- [ ] 无效参数给出友好提示
- [ ] 缺少依赖时给出明确错误信息
- [ ] 权限问题时建议解决方案
- [ ] 日志中 ERROR/WARN 信息清晰

### 性能表现
- [ ] 内存使用在合理范围
- [ ] 构建时间可接受
- [ ] 无明显内存泄漏
- [ ] CPU 利用率正常

---

## 🎯 第十步：下一步建议

### 如果所有测试通过：

1. **尝试不同预设**
   ```bash
   # Nightly 构建
   lmforge build iso arm64 nightly
   
   # Desktop 构建（需要充足资源）
   lmforge build iso amd64 desktop --desktop --live
   ```

2. **自定义配置**
   ```bash
   # 创建自定义预设
   cp presets/official.toml presets/my-custom.toml
   # 编辑 presets/my-custom.toml
   
   # 使用自定义预设
   lmforge build iso amd64 my-custom
   ```

3. **集成到 CI/CD**
   参考 `docs/LOGGING_EXAMPLES.md` 中的 GitHub Actions 示例

4. **贡献代码**
   - 报告 Bug: 检查 `output/build-*/logs/build.jsonl` 并提交 issue
   - 新功能: 先阅读 `docs/LOGGING_SYSTEM.md` 了解架构设计

### 如果遇到问题：

1. **收集诊断信息**
   ```bash
   # 收集日志
   tar czvf lmforge-debug.tar.gz ./output/build-*/logs/
   
   # 收集系统信息
   uname -a > system-info.txt
   cat /etc/os-release >> system-info.txt
   rustc --version >> system-info.txt
   cargo --version >> system-info.txt
   dpkg -l | grep -E "(debootstrap|live-build|squashfs)" >> system-info.txt
   ```

2. **查看详细文档**
   - [日志系统架构](docs/LOGGING_SYSTEM.md)
   - [日志输出示例](docs/LOGGING_EXAMPLES.md)
   - [官方预设说明](presets/README.md)

3. **寻求帮助**
   - 提供 `lmforge-debug.tar.gz` 和 `system-info.txt`
   - 描述复现步骤
   - 附上完整的错误输出

---

## 📝 快速参考卡

### 常用命令速查
```bash
# 编译
cargo build                    # 开发模式
cargo build --release          # 发布模式

# 基本操作
lmforge --help                 # 全局帮助
lmforge build --help           # 构建帮助
lmforge config --generate      # 生成配置
lmforge config --show          # 显示当前配置

# 构建命令
lmforge build rootfs i386 minimal                          # 最小 rootfs
lmforge build iso amd64 official                           # 官方 ISO
lmforge build iso amd64 desktop --desktop --live --installer # 完整桌面版

# 调试
RUST_LOG=debug lmforge build rootfs i386 minimal            # Debug 模式
lmforge build iso amd64 official --clean                   # 清理后重建

# 日志查看
cat output/build-*/logs/build.log                          # 主日志
jq '.' output/build-*/logs/build.jsonl | head              # JSONL 日志
cat output/build-*/logs/stages/workspace.log               # 阶段日志
```

### 关键路径
```
项目根目录:        /path/to/lmforge
可执行文件:        target/debug/lmforge (或 target/release/)
预设配置:          presets/*.toml
输出目录:          ./output (或 --output 指定)
工作区:            ./workspace (自动创建)
日志位置:          output/build-{ID}/logs/
构建产物:          output/build-{ID}/artifacts/
```

---

## 🎉 开始测试吧！

现在您已经拥有完整的测试指南。建议按照以下顺序进行：

1. **先运行基础测试**（第三步）- 5分钟内完成
2. **再尝试最小化构建**（第四步）- 约2-5分钟
3. **然后测试日志系统**（第六步）- 验证架构正确性
4. **最后挑战 ISO 构建**（第五步）- 需要10-30分钟

祝您测试顺利！如有任何问题，请参考故障排查章节或查看生成的详细日志。

**记住：File Logs = Truth，Console Output = UI** 🚀
