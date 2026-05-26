# Lingmo Forge Presets Configuration
# 
# This directory contains build presets for different use cases.
#
# Available Presets:
#   - official.toml    : Standard official ISO build (recommended for production)
#   - nightly.toml     : CI/CD nightly builds with debug logging
#   - minimal.toml     : Minimal base system (no desktop, no extras)
#   - desktop.toml     : Full desktop environment with all features
#
# Usage:
#   lmforge build iso <target> <preset-name>
#
# Examples:
#   lmforge build iso amd64 official
#   lmforge build iso arm64 nightly --desktop
#   lmforge build rootfs i386 minimal
#   lmforge build iso amd64 desktop --live --installer

# To create a custom preset, copy one of the existing files and modify it.
