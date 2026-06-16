use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use tracing::{info, debug, warn};

use crate::infra::workspace::WorkspaceLayout;
use crate::domain::context::BuildContext;

pub struct OverlayManager {
    workspace: Option<WorkspaceLayout>,
    packages: Vec<String>,
    custom_hooks: Vec<PathBuf>,
    includes_files: Vec<PathBuf>,
}

impl OverlayManager {
    pub fn new(workspace: &WorkspaceLayout) -> Self {
        OverlayManager {
            workspace: Some(workspace.clone()),
            packages: Vec::new(),
            custom_hooks: Vec::new(),
            includes_files: Vec::new(),
        }
    }

    pub fn with_packages(mut self, packages: &[&str]) -> Self {
        self.packages = packages.iter().map(|p| p.to_string()).collect();
        self
    }

    pub fn add_package(mut self, package: &str) -> Self {
        self.packages.push(package.to_string());
        self
    }

    pub fn add_hook(mut self, hook_path: &Path) -> Self {
        self.custom_hooks.push(hook_path.to_path_buf());
        self
    }

    pub fn add_includes_file(mut self, file_path: &Path) -> Self {
        self.includes_files.push(file_path.to_path_buf());
        self
    }

    pub fn initialize(&self) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        info!(target: "lmforge_overlay", "initializing overlay directories for V1 live-build integration");

        let dirs = [
            ("package-lists", workspace.overlay.join("package-lists")),
            ("includes.chroot", workspace.overlay.join("includes.chroot")),
            ("includes.binary", workspace.overlay.join("includes.binary")),
            ("hooks/chroot", workspace.overlay.join("hooks").join("chroot")),
            ("hooks/binary", workspace.overlay.join("hooks").join("binary")),
            ("filesystem", workspace.overlay.join("filesystem")),
            ("branding", workspace.overlay.join("branding")),
        ];

        for (name, path) in &dirs {
            std::fs::create_dir_all(path)
                .with_context(|| format!("Failed to create {} overlay directory: {:?}", name, path))?;
            
            debug!(target: "lmforge_overlay", directory = ?path, name = name, "created overlay directory");
        }

        self.create_default_package_list()?;
        self.create_default_branding()?;
        self.create_default_grub_theme()?;
        
        info!(target: "lmforge_overlay", "overlay directories initialized for V1 architecture");
        Ok(())
    }

    fn create_default_package_list(&self) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let pkg_list_dir = workspace.overlay.join("package-lists");
        std::fs::create_dir_all(&pkg_list_dir)?;

        let default_packages = r#"# Lingmo OS - Default Package List
# V1 Architecture: Packages installed via live-build

# Live system essentials
linux-image-amd64
live-boot
live-config
live-config-systemd
syslinux-common
pxelinux

# Desktop environment: Velora
velora-desktop-environment-base
velora-desktop-environment-core
velora-desktop-environment-extras
ddm
treeland
treeland-wayland-session
deepin-desktop-theme
velora-gtk-theme
deepin-wallpapers
lingmo-desktop-base
kwin-x11
kwin-wayland

# Network tools
network-manager
wpasupplicant
wireless-tools

# Filesystems support
dosfstools
ntfs-3g
exfat-fuse

# System utilities
sudo
locales
keyboard-configuration
console-setup

# Additional tools
vim-tiny
less
man-db
"#;

        let pkg_list_file = pkg_list_dir.join("lingmo-packages.list.chroot");
        if !pkg_list_file.exists() {
            std::fs::write(&pkg_list_file, default_packages)?;
            debug!(target: "lmforge_overlay", file = ?pkg_list_file, "created default package list");
        }

        Ok(())
    }

    fn create_default_branding(&self) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let branding_dir = workspace.overlay.join("branding");
        let branding_etc_dir = branding_dir.join("etc");
        
        std::fs::create_dir_all(&branding_etc_dir)?;

        let issue_content = r#"Lingmo OS Alpha \n \l
"#;
        std::fs::write(branding_etc_dir.join("issue"), issue_content)?;

        let issue_net_content = r#"Lingmo OS Alpha (\n) (\l)
"#;
        std::fs::write(branding_etc_dir.join("issue.net"), issue_net_content)?;

        let os_release_content = r#"PRETTY_NAME="Lingmo OS"
NAME="Lingmo OS"
VERSION_ID=5.0
VERSION="5.0 (Alpha)"
ID=lingmo
ID_LIKE=debian
HOME_URL="https://www.lingmo.org"
SUPPORT_URL="https://www.lingmo.org/support"
BUG_REPORT_URL="https://bugs.lingmo.org"
"#;
        std::fs::write(branding_etc_dir.join("os-release"), os_release_content)?;

        debug!(target: "lmforge_overlay", "created default branding files");

        Ok(())
    }

    pub fn sync_to_livebuild(&self, lb_config: &Path, ctx: &BuildContext) -> Result<()> {
        info!(
            target: "lmforge_overlay",
            lb_config = ?lb_config,
            stage = "sync",
            "V1 Phase: Synchronizing overlays to live-build configuration"
        );

        let config_dir = lb_config.join("config");
        std::fs::create_dir_all(&config_dir)?;

        self.sync_package_lists(&config_dir)?;
        self.sync_repositories(&config_dir, ctx)?;
        self.sync_includes_chroot(&config_dir)?;
        self.sync_includes_binary(&config_dir)?;
        self.sync_hooks(&config_dir)?;
        self.sync_customizations(ctx, &config_dir)?;

        info!(
            target: "lmforge_overlay",
            stage = "sync",
            "Overlay synchronization completed successfully"
        );

        Ok(())
    }

    fn sync_package_lists(&self, config_dir: &Path) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let lists_dir = config_dir.join("package-lists");
        std::fs::create_dir_all(&lists_dir)?;

        let source_pkg_dir = workspace.overlay.join("package-lists");

        if source_pkg_dir.exists() {
            for entry in std::fs::read_dir(&source_pkg_dir)? {
                let entry = entry?;
                let src_file = entry.path();
                
                if src_file.is_file() && src_file.extension().map(|e| e == "chroot" || e == "list").unwrap_or(false) {
                    let filename = src_file.file_name()
                        .context("Invalid package list filename")?
                        .to_string_lossy()
                        .to_string();
                    
                    let dest_file = lists_dir.join(&filename);
                    
                    std::fs::copy(&src_file, &dest_file)
                        .with_context(|| format!("Failed to copy package list {:?} to {:?}", src_file, dest_file))?;

                    debug!(
                        target: "lmforge_overlay",
                        file = %filename,
                        dest = ?dest_file,
                        "synced package list to live-build"
                    );
                }
            }
        } else {
            warn!(target: "lmforge_overlay", "no package-lists directory found in overlay");
        }

        info!(target: "lmforge_overlay", "package lists synchronized");

        Ok(())
    }

    fn sync_repositories(&self, config_dir: &Path, ctx: &BuildContext) -> Result<()> {
        let repos = &ctx.config.repositories;
        if repos.is_empty() {
            return Ok(());
        }

        let archives_dir = config_dir.join("archives");
        std::fs::create_dir_all(&archives_dir)?;

        for repo in repos {
            if repo.enabled == Some(false) {
                continue;
            }

            // Write archives/*.list.chroot (no signed-by: let lb auto-handle key from .key.chroot)
            let list_content = format!("deb {} {}\n", repo.uri, repo.suite);
            let list_file = archives_dir.join("lingmo.list.chroot");
            std::fs::write(&list_file, list_content)?;
            debug!(target: "lmforge_overlay", file = ?list_file, repo = %repo.name, "wrote archive list");

            // Copy binary GPG key to config/archives/lingmo.key.chroot
            // Per live-build manual §8.1.5: config/archives/{name}.key.{chroot,binary}
            let key_file = archives_dir.join("lingmo.key.chroot");
            let local_key = PathBuf::from("assets/repositories").join("lingmo.key");

            if !local_key.exists() {
                anyhow::bail!(
                    "GPG key file not found at {:?}. Repository '{}' requires signing key.",
                    local_key, repo.name
                );
            }

            std::fs::copy(&local_key, &key_file)
                .with_context(|| format!("Failed to copy GPG key {:?} -> {:?}", local_key, key_file))?;
            info!(target: "lmforge_overlay", src = ?local_key, dst = ?key_file, "copied signing key to archives");
        }

        info!(target: "lmforge_overlay", count = repos.len(), "repositories synchronized");
        Ok(())
    }

    fn sync_includes_chroot(&self, config_dir: &Path) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let includes_chroot = config_dir.join("includes.chroot");
        std::fs::create_dir_all(&includes_chroot)?;

        let overlay_sources = [
            ("filesystem", workspace.overlay.join("filesystem")),
            ("branding", workspace.overlay.join("branding")),
        ];

        for (name, source) in &overlay_sources {
            if source.exists() {
                self.copy_recursive(source, &includes_chroot)?;
                debug!(
                    target: "lmforge_overlay",
                    overlay_name = name,
                    source = ?source,
                    "synced include files"
                );
            }
        }

        for custom_file in &self.includes_files {
            if custom_file.exists() {
                let filename = custom_file.file_name()
                    .context("Invalid custom includes filename")?
                    .to_string_lossy()
                    .to_string();
                
                let dest_path = includes_chroot.join(&filename);
                
                std::fs::copy(custom_file, &dest_path)
                    .with_context(|| format!("Failed to copy custom include {:?} to {:?}", custom_file, dest_path))?;

                debug!(
                    target: "lmforge_overlay",
                    file = %filename,
                    "synced custom include file"
                );
            }
        }

        info!(target: "lmforge_overlay", "includes.chroot synchronized");

        Ok(())
    }

    fn sync_includes_binary(&self, config_dir: &Path) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let includes_binary = config_dir.join("includes.binary");
        let source = workspace.overlay.join("includes.binary");

        if source.exists() {
            std::fs::create_dir_all(&includes_binary)?;
            self.copy_recursive(source, &includes_binary)?;
            info!(target: "lmforge_overlay", "includes.binary synchronized (GRUB theme, etc.)");
        }

        Ok(())
    }

    fn create_default_grub_theme(&self) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let theme_dir = workspace.overlay.join("includes.binary").join("boot").join("grub").join("theme");
        std::fs::create_dir_all(&theme_dir)?;

        // Lingmo OS GRUB theme
        let theme_txt = r#"# Lingmo OS GRUB Theme
title-text: "Lingmo OS"
title-color: "#FFFFFF"
title-font: "Sans Bold 24"
desktop-image: "background.png"
desktop-color: "#1a1a2e"

terminal-left: "5%"
terminal-top: "80%"
terminal-width: "90%"
terminal-height: "15%"
terminal-border: "0"
terminal-font: "Mono 12"

+ label {
    left: 5%
    top: 10%
    width: 90%
    height: 30%
    text-align: center
    font: "Sans Bold 36"
    color: "#FFFFFF"
}
+ boot_menu {
    left: 15%
    top: 45%
    width: 70%
    height: 40%
    item_color="#CCCCCC"
    selected_item_color="#FFFFFF"
    icon_width=32
    icon_height=32
    item_font="Sans 16"
    selected_item_font="Sans Bold 16"
    item_height=28
    item_padding=4
    item_icon_space=20
    item_spacing=8
}
+ status {
    left: 5%
    bottom: 3%
    width: 90%
    height: 25px
    font = "Sans 12"
    color = "#888888"
    text-align = center
}
"#;

        std::fs::write(theme_dir.join("theme.txt"), theme_txt)?;

        info!(target: "lmforge_overlay", path = ?theme_dir, "created default GRUB theme");
        Ok(())
    }

    fn sync_hooks(&self, config_dir: &Path) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let hook_types = [
            ("hooks/chroot", "chroot"),
            ("hooks/binary", "binary"),
        ];

        for (source_rel, _hook_type) in &hook_types {
            let hooks_source = workspace.overlay.join(source_rel);
            let hooks_dest = config_dir.join("hooks");

            if !hooks_source.exists() {
                continue;
            }

            std::fs::create_dir_all(&hooks_dest)?;

            for entry in std::fs::read_dir(&hooks_source)? {
                let entry = entry?;
                let source_path = entry.path();

                if source_path.is_file() {
                    let filename = source_path.file_name()
                        .context("Invalid hook filename")?
                        .to_string_lossy()
                        .to_string();

                    let dest_path = hooks_dest.join(&filename);

                    std::fs::copy(&source_path, &dest_path)
                        .with_context(|| format!("Failed to copy hook {:?} to {:?}", source_path, dest_path))?;

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(metadata) = std::fs::metadata(&dest_path) {
                            let mut perms = metadata.permissions();
                            perms.set_mode(0o755);
                            let _ = std::fs::set_permissions(&dest_path, perms);
                        }
                    }

                    debug!(
                        target: "lmforge_overlay",
                        hook_name = %filename,
                        "installed hook script"
                    );
                }
            }
        }

        for custom_hook in &self.custom_hooks {
            if custom_hook.exists() {
                let filename = custom_hook.file_name()
                    .context("Invalid custom hook filename")?
                    .to_string_lossy()
                    .to_string();

                let hooks_dest = config_dir.join("hooks");
                std::fs::create_dir_all(&hooks_dest)?;

                let dest_path = hooks_dest.join(&filename);

                std::fs::copy(custom_hook, &dest_path)
                    .with_context(|| format!("Failed to copy custom hook {:?} to {:?}", custom_hook, dest_path))?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = std::fs::metadata(&dest_path) {
                        let mut perms = metadata.permissions();
                        perms.set_mode(0o755);
                        let _ = std::fs::set_permissions(&dest_path, perms);
                    }
                }

                debug!(
                    target: "lmforge_overlay",
                    hook_name = %filename,
                    "installed custom hook"
                );
            }
        }

        info!(target: "lmforge_overlay", "hooks synchronized");

        Ok(())
    }

    fn sync_customizations(&self, ctx: &BuildContext, config_dir: &Path) -> Result<()> {
        let auto_config = config_dir.join("auto");
        std::fs::create_dir_all(&auto_config)?;

        let auto_config_content = format!(
            r#"#!/bin/sh
set -e

echo ">>> [lmforge] Applying V1 customizations <<<"

LB_ARCHITECTURE="{arch}"
LB_DISTRIBUTION="{suite}"
LB_ARCHIVE_AREAS="{components}"
LB_PARENT_ARCHIVE_AREAS="{components}"

LB_BOOTLOADER="grub-efi"
LB_CHROOT_FILESYSTEM="squashfs"
LB_BINARY_FILESYSTEM="fat32"

LB_APPLICATION_TITLE="Lingmo OS"
LB_ISO_NAME="lingmo-os"
LB_ISO_VOLUME="Lingmo OS Live {version}"

echo ">>> [lmforge] Configuration loaded for {suite} ({arch}) <<<"
"#,
            arch = ctx.arch(),
            suite = ctx.suite(),
            components = ctx.config.platform.components.join(" "),
            version = env!("CARGO_PKG_VERSION")
        );

        let config_file = auto_config.join("config");
        std::fs::write(&config_file, auto_config_content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(&config_file) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&config_file, perms);
            }
        }

        debug!(target: "lmforge_overlay", file = ?config_file, "generated live-build auto configuration");

        info!(target: "lmforge_overlay", "customizations applied");

        Ok(())
    }

    pub fn load_package_list(&self) -> Result<Vec<String>> {
        let pkg_file = match &self.workspace {
            Some(ws) => ws.overlay.join("package-lists").join("lingmo-packages.list.chroot"),
            None => return Ok(Vec::new()),
        };
        
        if !pkg_file.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&pkg_file)?;
        let packages: Vec<String> = content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();

        debug!(
            target: "lmforge_overlay",
            count = packages.len(),
            "loaded package list from overlay"
        );

        Ok(packages)
    }

    fn copy_recursive(&self, source: &Path, target: &Path) -> Result<()> {
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let src_path = entry.path();
            let rel_path = src_path.strip_prefix(source)?;
            let dst_path = target.join(rel_path);

            if src_path.is_dir() {
                std::fs::create_dir_all(&dst_path)?;
                self.copy_recursive(&src_path, &dst_path)?;
            } else if src_path.is_file() {
                if let Some(parent) = dst_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                
                std::fs::copy(&src_path, &dst_path)
                    .with_context(|| format!("Failed to copy {:?} to {:?}", src_path, dst_path))?;
                
                debug!(
                    target: "lmforge_overlay",
                    source = ?src_path,
                    dest = ?dst_path,
                    "copied file during overlay sync"
                );
            }
        }

        Ok(())
    }
}
