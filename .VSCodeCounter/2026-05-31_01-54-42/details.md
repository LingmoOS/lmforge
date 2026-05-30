# Details

Date : 2026-05-31 01:54:42

Directory d:\\Projects\\rust\\lmforge

Total : 51 files,  8625 codes, 22 comments, 1985 blanks, all 10632 lines

[Summary](results.md) / Details / [Diff Summary](diff.md) / [Diff Details](diff-details.md)

## Files
| filename | language | code | comment | blank | total |
| :--- | :--- | ---: | ---: | ---: | ---: |
| [BUILD\_STATUS.md](/BUILD_STATUS.md) | Markdown | 390 | 0 | 94 | 484 |
| [README.md](/README.md) | Markdown | 165 | 0 | 55 | 220 |
| [README\_zh.md](/README_zh.md) | Markdown | 165 | 0 | 55 | 220 |
| [docs/DEBIAN\_TESTING\_GUIDE.md](/docs/DEBIAN_TESTING_GUIDE.md) | Markdown | 547 | 0 | 152 | 699 |
| [docs/LOGGING\_EXAMPLES.md](/docs/LOGGING_EXAMPLES.md) | Markdown | 314 | 0 | 69 | 383 |
| [docs/LOGGING\_SYSTEM.md](/docs/LOGGING_SYSTEM.md) | Markdown | 342 | 0 | 77 | 419 |
| [presets/README.md](/presets/README.md) | Markdown | 19 | 0 | 2 | 21 |
| [src/command/build.rs](/src/command/build.rs) | Rust | 54 | 2 | 19 | 75 |
| [src/command/cli.rs](/src/command/cli.rs) | Rust | 134 | 10 | 36 | 180 |
| [src/command/mod.rs](/src/command/mod.rs) | Rust | 3 | 0 | 1 | 4 |
| [src/command/package.rs](/src/command/package.rs) | Rust | 54 | 9 | 13 | 76 |
| [src/domain/artifact.rs](/src/domain/artifact.rs) | Rust | 130 | 0 | 19 | 149 |
| [src/domain/config.rs](/src/domain/config.rs) | Rust | 185 | 0 | 28 | 213 |
| [src/domain/context.rs](/src/domain/context.rs) | Rust | 191 | 0 | 31 | 222 |
| [src/domain/mod.rs](/src/domain/mod.rs) | Rust | 3 | 0 | 1 | 4 |
| [src/engine/engine\_trait.rs](/src/engine/engine_trait.rs) | Rust | 10 | 0 | 6 | 16 |
| [src/engine/livebuild.rs](/src/engine/livebuild.rs) | Rust | 399 | 0 | 112 | 511 |
| [src/engine/mod.rs](/src/engine/mod.rs) | Rust | 3 | 0 | 1 | 4 |
| [src/engine/orchestrator.rs](/src/engine/orchestrator.rs) | Rust | 633 | 0 | 177 | 810 |
| [src/features/desktop.rs](/src/features/desktop.rs) | Rust | 91 | 0 | 25 | 116 |
| [src/features/feature\_trait.rs](/src/features/feature_trait.rs) | Rust | 20 | 0 | 7 | 27 |
| [src/features/installer.rs](/src/features/installer.rs) | Rust | 61 | 0 | 23 | 84 |
| [src/features/live.rs](/src/features/live.rs) | Rust | 76 | 0 | 29 | 105 |
| [src/features/mod.rs](/src/features/mod.rs) | Rust | 4 | 0 | 1 | 5 |
| [src/infra/artifact\_manager.rs](/src/infra/artifact_manager.rs) | Rust | 343 | 0 | 60 | 403 |
| [src/infra/checksum.rs](/src/infra/checksum.rs) | Rust | 48 | 0 | 19 | 67 |
| [src/infra/cleanup.rs](/src/infra/cleanup.rs) | Rust | 847 | 0 | 155 | 1,002 |
| [src/infra/mod.rs](/src/infra/mod.rs) | Rust | 9 | 0 | 2 | 11 |
| [src/infra/overlay.rs](/src/infra/overlay.rs) | Rust | 376 | 0 | 100 | 476 |
| [src/infra/workspace.rs](/src/infra/workspace.rs) | Rust | 190 | 0 | 39 | 229 |
| [src/main.rs](/src/main.rs) | Rust | 28 | 0 | 9 | 37 |
| [src/platform/debian.rs](/src/platform/debian.rs) | Rust | 152 | 0 | 34 | 186 |
| [src/platform/mod.rs](/src/platform/mod.rs) | Rust | 2 | 0 | 1 | 3 |
| [src/platform/platform\_trait.rs](/src/platform/platform_trait.rs) | Rust | 13 | 0 | 10 | 23 |
| [src/runtime/log\_stream.rs](/src/runtime/log_stream.rs) | Rust | 602 | 0 | 104 | 706 |
| [src/runtime/mod.rs](/src/runtime/mod.rs) | Rust | 8 | 0 | 2 | 10 |
| [src/runtime/mount.rs](/src/runtime/mount.rs) | Rust | 159 | 0 | 37 | 196 |
| [src/runtime/mount\_manager.rs](/src/runtime/mount_manager.rs) | Rust | 510 | 0 | 105 | 615 |
| [src/runtime/process.rs](/src/runtime/process.rs) | Rust | 326 | 0 | 66 | 392 |
| [src/runtime/sandbox.rs](/src/runtime/sandbox.rs) | Rust | 112 | 1 | 26 | 139 |
| [src/runtime/signal\_handler.rs](/src/runtime/signal_handler.rs) | Rust | 162 | 0 | 36 | 198 |
| [src/stages/mod.rs](/src/stages/mod.rs) | Rust | 2 | 0 | 1 | 3 |
| [src/stages/pipeline.rs](/src/stages/pipeline.rs) | Rust | 108 | 0 | 27 | 135 |
| [src/stages/stage.rs](/src/stages/stage.rs) | Rust | 26 | 0 | 9 | 35 |
| [src/telemetry/build\_id.rs](/src/telemetry/build_id.rs) | Rust | 67 | 0 | 17 | 84 |
| [src/telemetry/console.rs](/src/telemetry/console.rs) | Rust | 107 | 0 | 16 | 123 |
| [src/telemetry/context.rs](/src/telemetry/context.rs) | Rust | 58 | 0 | 10 | 68 |
| [src/telemetry/file\_logger.rs](/src/telemetry/file_logger.rs) | Rust | 147 | 0 | 27 | 174 |
| [src/telemetry/layer.rs](/src/telemetry/layer.rs) | Rust | 29 | 0 | 7 | 36 |
| [src/telemetry/mod.rs](/src/telemetry/mod.rs) | Rust | 22 | 0 | 5 | 27 |
| [src/telemetry/runtime.rs](/src/telemetry/runtime.rs) | Rust | 179 | 0 | 28 | 207 |

[Summary](results.md) / Details / [Diff Summary](diff.md) / [Diff Details](diff-details.md)