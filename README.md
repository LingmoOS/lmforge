# lmforge

LingmoOS Build Tool (lmforge) - Linux distribution build tool.

## Overview

lmforge handles the compilation of Linux distribution images. It manages build workspaces, configuration, runtime execution, and artifact collection.

The project targets Debian-based distributions. The current implementation uses live-build as the backend engine. lmforge handles build coordination, workspace lifecycle, overlay management, and cleanup/recovery.

## Architecture

```
lmforge (build layer)
  ├── WorkspaceManager  (build isolation)
  ├── LiveBuildEngine   (live-build wrapper)
  ├── OverlayManager    (filesystem/branding/hooks)
  ├── ArtifactManager   (ISO + checksums + manifest)
  └── CleanupRecovery   (failure recovery + stale cleanup)

Runtime layer
  └── ProcessRunner     (process execution, stdout/stderr capture)

Backend
  └── live-build        (lb config / lb build / lb clean)
      └── debootstrap / mmdebstrap
```

Build coordination uses synchronous interfaces. Runtime internals use tokio for async operations where needed.

## Features

- **CLI interface** for build, config, and package operations
- **Workspace isolation** with independent build directories per run
- **Configuration system** with presets and user overrides
- **Live-build integration**: config, build, and clean lifecycle management
- **Overlay support**: filesystem overlays, branding customization, hook injection, package lists
- **Artifact collection**: ISO gathering, SHA256 checksum generation, build manifest creation
- **Cleanup and recovery**: lock files, interrupted build detection, stale workspace removal
- **Structured logging** with stage-level output

## Build Flow

```
1. Load configuration (presets → overrides → user config)
2. Initialize workspace (output/build-<timestamp>-<build-id>/)
3. Setup cleanup recovery (lock file, PID tracking)
4. Execute pipeline stages:
   - workspace: prepare build directory
   - bootstrap: base system via debootstrap/mmdebstrap
   - packages: install packages into rootfs
   - overlay: apply filesystem/branding/customizations
   - image: run live-build to generate ISO
   - metadata: generate manifest and checksums
   - release: collect artifacts to output directory
5. Mark build completed or trigger failure recovery
6. Cleanup temporary files
```

Output structure:
```
output/build-20260529-abcdef1234/
├── config/           # live-build configuration
├── cache/            # cached downloads
├── artifacts/        # final ISO + checksums + manifest
│   ├── lingmo-live.iso
│   ├── SHA256SUMS
│   └── build-manifest.json
├── logs/
│   └── stages/       # per-stage log files
├── runtime/          # runtime state
├── temp/             # temporary files
├── rootfs/           # root filesystem
└── overlay/          # custom overlays
    ├── filesystem/
    ├── branding/
    ├── packages.list
    └── hooks/
```

## Installation

### Requirements

- Rust toolchain (1.70+)
- For building images: live-build, debootstrap or mmdebstrap

### Build from source

```bash
git clone <repository-url>
cd lmforge
cargo build --release
```

Binary location: `target/release/lmforge`

## Usage

### Build an ISO image

```bash
# Basic build with default configuration
lmforge build iso official

# Verbose output with stage details
lmforge build iso official -v

# Dry-run without executing commands
lmforge build iso official --dry-run

# Custom output directory
lmforge build iso official --output ./my-builds

# Override architecture and suite
lmforge build iso official --arch amd64 --suite bookworm
```

### Show current configuration

```bash
# Display resolved configuration
lmforge config --show

# Use custom config file
lmforge config --show --config ./my-config.toml
```

### Build options

```
Options:
  -v, --verbose       Enable verbose output
  --config <PATH>     Configuration file path
  --output <PATH>     Output directory
  --workspace <PATH>  Workspace base directory
  --arch <ARCH>       Target architecture (amd64, arm64)
  --suite <SUITE>     Debian suite (bookworm, trixie, sid)
```

## Configuration

Configuration is loaded in this order (later values override earlier):

1. **Built-in presets**: Default settings for known distributions
2. **Override files**: Project-specific configurations
3. **User config**: `--config` flag or default paths

Example configuration (`lingmo-official.toml`):
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

### Overlays

Place custom files in the overlay directories:

```
overlay/
├── filesystem/         # Files copied into rootfs
│   └── etc/
│       └── custom.conf
├── branding/           # Distribution branding
│   └── etc/
│       ├── issue
│       └── os-release
├── packages.list       # Additional packages (one per line)
└── hooks/              # Live-build hooks
    ├── 01-custom.chroot
    └── 02-postinstall.chroot
```

These are automatically synchronized to the live-build configuration during builds.

## Project Status

**Early development**

- API is unstable and may change between versions
- Currently focused on Debian/bookworm with live-build backend
- Runtime lifecycle management is being refined
- Testing coverage is limited
- Documentation is incomplete

This is not production-ready software. Use for development and testing only.

## Development

### Running tests

```bash
cargo test
```

### Building with debug output

```bash
RUST_LOG=debug cargo run -- build iso official -v
```

## License

GPL-2.0 License. See [LICENSE](LICENSE) for details.
