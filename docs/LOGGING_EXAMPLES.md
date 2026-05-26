# Console Output Examples

## Successful Build

```bash
$ lmforge build iso amd64 official --desktop

lmforge v0.1.0 starting
build_id=build-20260527-a1b2c3d4 version=0.1.0

[workspace] preparing build workspace
[workspace] bootstrapping Debian bookworm (amd64)
[workspace] mounting filesystems for chroot
[packages ] resolving desktop profile
[packages ] installing 156 packages
[packages ] WARN: firmware-linux-nonfree: package not found in repos
[overlay  ] applying branding overlay
[overlay  ] executing 3 hooks
[image    ] preparing live-build environment
[image    ] generating squashfs
[image    ] generating EFI bootloader
[image    ] generating ISO image
[metadata] generating MANIFEST
[metadata] writing SHA256SUMS
[release  ] writing BUILDINFO.json
[release  ] complete

Build completed:
  build_id: build-20260527-a1b2c3d4
  duration: 12m 34s
  stages: 6/6
  artifacts: 3
    - lingmo-live-amd64.iso (2.3 GB)
    - SHA256SUMS (256 B)
    - BUILDINFO.json (1.2 KB)

Output: ./output/build-20260527-a1b2c3d4/artifacts/
Logs:   ./output/build-20260527-a1b2c3d4/logs/
```

## Build with Errors

```bash
$ lmforge build iso arm64 nightly

lmforge v0.1.0 starting
build_id=build-20260528-e5f6g7h8

[workspace] preparing build workspace
[workspace] bootstrapping Debian sid (arm64)
[packages ] installing base packages
[overlay  ] applying filesystem overlay
[image    ] generating squashfs
[image    ] ERROR: mksquashfs exited with status 1
[image    ] failed: mksquashfs out of memory (need 8GB, have 4GB)

Build FAILED:
  stage: image
  error: mksquashfs out of memory
  build_id: build-20260528-e5f6g7h8
  
Check logs for details:
  ./output/build-20260528-e5f6g7h8/logs/image.log
  ./output/build-20260528-e5f6g7h8/logs/build.jsonl
```

## Minimal Build (Verbose Debug Mode)

```bash
$ RUST_LOG=debug lmforge build rootfs i386 minimal

lmforge v0.1.0 starting
build_id=build-20260529-i7j8k9l0

[workspace] preparing build workspace
[workspace] creating directory structure
[workspace] bootstrapping Debian bookworm (i386) [variant=minbase]
[packages ] no additional packages to install
[release  ] complete

Build completed:
  artifacts: 1
    - rootfs.tar.zst (156 MB)
```

---

# File Log Examples

## build.log (Text Format)

```
2026-05-27T12:00:00Z INFO workspace: lmforge v0.1.0 starting build_id=build-20260527-a1b2c3d4
2026-05-27T12:00:01Z INFO workspace: starting build orchestration target=iso build_id=build-20260527-a1b2c3d4
2026-05-27T12:00:02Z INFO workspace: loading configuration
2026-05-27T12:00:02Z DEBUG workspace: configuration loaded from presets/official.toml
2026-05-27T12:00:03Z INFO workspace: workspace create: ./workspace
2026-05-27T12:00:03Z INFO workspace: build context initialized arch=amd64 suite=bookworm output=./output
2026-05-27T12:00:04Z INFO workspace: creating platform instance platform_name=debian
2026-05-27T12:00:04Z INFO workspace: validating debian platform environment
2026-05-27T12:00:05Z INFO runtime: exec: debootstrap --arch=amd64 --variant=minbase bookworm ./workspace/rootfs http://deb.debian.org/debian main contrib nonfree
2026-05-27T12:01:30Z DEBUG runtime: process completed: debootstrap (exit=0) duration_ms=85000 stdout_len=2048 stderr_len=512
2026-05-27T12:01:30Z INFO runtime: mount: /proc -> ./workspace/rootfs/proc (proc)
2026-05-27T12:01:31Z INFO runtime: mount: /sys -> ./workspace/rootfs/sys (sysfs)
2026-05-27T12:01:31Z INFO runtime: mount: tmpfs -> ./workspace/rootfs/run (tmpfs)
2026-05-27T12:01:32Z INFO packages: installing base packages
2026-05-27T12:02:15Z INFO runtime: exec: chroot ./workspace/rootfs apt-get install -y linux-image-amd64 initramfs-tools grub-efi-amd64
2026-05-27T12:03:45Z DEBUG runtime: process completed: apt-get (exit=0) duration_ms=90000 stdout_len=4096 stderr_len=1024
2026-05-27T12:03:46Z INFO overlay: applying overlays to rootfs
2026-05-27T12:03:47Z INFO overlay: copied 23 files from branding overlay
2026-05-27T12:03:48Z INFO overlay: executed hook: 99-cleanup.sh
2026-05-27T12:03:49Z INFO image: generating image with engine: live-build
2026-05-27T12:04:00Z INFO runtime: exec: lb config
2026-05-27T12:04:30Z DEBUG runtime: process completed: lb (exit=0) duration_ms=30000
2026-05-27T12:04:31Z INFO runtime: exec: lb build
2026-05-27T12:10:22Z DEBUG runtime: process completed: lb (exit=0) duration_ms=351000 stdout_len=16384 stderr_len=4096
2026-05-27T12:10:23Z INFO metadata: generating manifest and checksums
2026-05-27T12:10:24Z INFO metadata: manifest written to ./output/MANIFEST
2026-05-27T12:10:25Z INFO release: release finalized artifacts_count=3 stages=["workspace","packages","overlay","image","metadata","release"]
2026-05-27T12:10:26Z INFO release: build completed successfully stages_completed=6 total_stages=6 duration_secs=626.0
```

## build.jsonl (Structured Format)

```json
{"timestamp":"2026-05-27T12:00:00Z","level":"INFO","stage":"workspace","message":"lmforge v0.1.0 starting","build_id":"build-20260527-a1b2c3d4","target":"lmforge::main"}
{"timestamp":"2026-05-27T12:00:01Z","level":"INFO","stage":"workspace","message":"starting build orchestration","build_id":"build-20260527-a1b2c3d4","target":"lmforge_workspace","target_name":"iso"}
{"timestamp":"2026-05-27T12:00:05Z","level":"INFO","stage":"runtime","message":"exec: debootstrap --arch=amd64 --variant=minbase bookworm ./workspace/rootfs http://deb.debian.org/debian main contrib nonfree","build_id":"build-20260527-a1b2c3d4","command":"debootstrap","args_count":9,"working_dir":"/tmp"}
{"timestamp":"2026-05-27T12:01:30Z","level":"DEBUG","stage":"runtime","message":"process completed: debootstrap (exit=0)","build_id":"build-20260527-a1b2c3d4","command":"debootstrap","exit_code":0,"duration_ms":85000,"stdout_len":2048,"stderr_len":512}
{"timestamp":"2026-05-27T12:01:30Z","level":"INFO","stage":"runtime","message":"mount: /proc -> ./workspace/rootfs/proc (proc)","build_id":"build-20260527-a1b2c3d4","source":"/proc","target":"./workspace/rootfs/proc","fs_type":"proc"}
{"timestamp":"2026-05-27T12:10:26Z","level":"INFO","stage":"release","message":"build completed successfully","build_id":"build-20260527-a1b2c3d4","stages_completed":6,"total_stages":6,"duration_secs":626.0}
```

## stages/workspace.log (Stage-specific Log)

```
2026-05-27T12:00:02Z INFO : loading configuration
2026-05-27T12:00:03Z INFO : workspace create: ./workspace
2026-05-27T12:00:03Z INFO : build context initialized arch=amd64 suite=bookworm
2026-05-27T12:00:04Z INFO : creating platform instance platform_name=debian
2026-05-27T12:00:04Z INFO : validating debian platform environment
2026-05-27T12:00:05Z INFO : stage start: workspace
2026-05-27T12:00:06Z DEBUG: exec: debootstrap --arch=amd64 --variant=minbase bookworm ...
2026-05-27T12:01:30Z DEBUG: stdout: I: Retrieving InRelease ...
2026-05-27T12:01:30Z DEBUG: stdout: I: Retrieving Packages ...
2026-05-27T12:01:30Z DEBUG: stdout: I: Base system installed successfully.
2026-05-27T12:01:30Z DEBUG: stderr: W: Size mismatch for some packages...
2026-05-27T12:01:31Z INFO : mount: proc -> rootfs/proc
2026-05-27T12:01:32Z INFO : mount: sysfs -> rootfs/sys
2026-05-27T12:01:33Z INFO : mount: tmpfs -> rootfs/run
2026-05-27T12:01:34Z INFO : stage complete: workspace (90.2s)
```

## stages/image.log (Stage with Subprocess Details)

```
2026-05-27T12:03:49Z INFO : stage start: image
2026-05-27T12:03:50Z INFO : generating image with engine: live-build
2026-05-27T12:04:00Z DEBUG: exec: lb config
2026-05-27T12:04:30Z DEBUG: process completed: lb config (exit=0) duration=30.0s
2026-05-27T12:04:31Z DEBUG: 
=== STDOUT (lb config) ===
P: Creating config tree...
P: Setting up debian-installer...
P: Configuring architecture...
=== END STDOUT ===

2026-05-27T12:04:31Z DEBUG: 
=== STDERR (lb config) ===
W: Some warnings during configuration
=== END STDERR ===

2026-05-27T12:04:32Z DEBUG: exec: lb build
2026-05-27T12:09:43Z DEBUG: process completed: lb build (exit=0) duration=311.2s
2026-05-27T12:09:44Z DEBUG: 
=== STDOUT (lb build) ===
P: Building binary squashfs image...
P: Creating filesystem.squashfs...
Parallel mksquashfs: Using 16 processors
...
P: Binary image build complete.
=== END STDOUT ===

2026-05-27T12:09:45Z DEBUG: 
=== STDERR (lb build) ===
warning: xattr unsupported on target filesystem
I: Calculating checksums...
=== END STDERR ===

2026-05-27T12:09:46Z INFO : generated artifact: lingmo-live-amd64.iso (2457632768 bytes)
2026-05-27T12:09:47Z INFO : stage complete: image (358.0s)
```

---

# Error Scenario Logs

## Image Generation Failure

### Console Output
```text
[workspace] preparing build workspace
[workspace] bootstrapping Debian bookworm (amd64)
[packages ] installing base packages
[overlay  ] applying filesystem overlay
[image    ] generating squashfs
[image    ] ERROR: mksquashfs failed with exit code 1
[image    ] failed: mksquashfs: error allocating memory (8GB required, 4GB available)

Build FAILED:
  Check full logs: ./output/build-20260528-bad1/build.jsonl
  Stage log:      ./output/build-20260528-bad1/logs/image.log
```

### File Log (image.log)
```
2026-05-28T14:20:00Z ERROR: stage start: image
2026-05-28T14:20:01Z ERROR: generating squashfs
2026-05-28T14:20:02Z DEBUG: exec: mksquashfs rootfs filesystem.squashfs -comp zstd -Xcompression-level 19
2026-05-28T14:22:33Z DEBUG: process completed: mksquashfs (exit=1) duration=151.2s
2026-05-28T14:22:34Z ERROR: 
=== STDERR ===
mksquashfs: error allocating 8589934592 bytes for compressor buffer
mksquashfs: only 4294967296 bytes available
Aborted (core dumped)
=== END STDERR ===

2026-05-28T14:22:35Z ERROR: stage error: image
error="mksquashfs: out of memory"
command="mksquashfs"
exit_code=1
duration_ms=151200
```

### JSONL Entry
```json
{
  "timestamp": "2026-05-28T14:22:34Z",
  "level": "ERROR",
  "stage": "image",
  "message": "stage error: image",
  "error": "mksquashfs: out of memory",
  "command": "mksquashfs",
  "exit_code": 1,
  "duration_ms": 151200,
  "build_id": "build-20260528-bad1"
}
```

---

# Performance Metrics Example

## Timing Summary (from logs)

```
Build ID:     build-20260527-perf
Total Time:   18m 42.3s
Stages:       6/6 successful

Stage Breakdown:
  workspace  [02:15.3s] ████████████████████░░░░  12.1%
  packages  [05:42.1s] ██████████████████████████  30.5%
  overlay   [00:08.2s] █░░░░░░░░░░░░░░░░░░░░░░░   0.7%
  image     [09:56.4s] ████████████████████████████████████████  53.1%
  metadata  [00:03.1s] ░░░░░░░░░░░░░░░░░░░░░░░░   0.3%
  release   [00:37.2s] ██░░░░░░░░░░░░░░░░░░░░░░   3.3%

Subprocess Execution:
  debootstrap          85.2s  (exit 0)
  apt-get install      92.4s  (exit 0)
  lb config            30.1s  (exit 0)
  lb build            351.8s  (exit 0)
  mksquashfs          142.6s  (exit 0)
  grub-mkstandalone    12.3s  (exit 0)
  xorriso              89.4s  (exit 0)

Mount Operations:
  proc    mounted  at 12:01:30
  sysfs   mounted  at 12:01:31
  tmpfs   mounted  at 12:01:32
  dev     bind     at 12:01:33
  devpts  mounted  at 12:01:34
  run     mounted  at 12:01:35
  
  All unmounted at 12:10:22

Artifacts Generated:
  lingmo-live-amd64.iso   2,457,632,768 bytes  sha256:a1b2c3d4...
  SHA256SUMS              256 bytes           sha256:e5f6g7h8...
  BUILDINFO.json          1,234 bytes          -
```

---

# CI/CD Integration Example

## GitHub Actions Log Parsing

```yaml
# .github/workflows/build.yml
- name: Build ISO
  run: |
    lmforge build iso amd64 official --output ./dist
    
- name: Parse Build Logs
  if: always()
  run: |
    # Extract build info
    BUILD_ID=$(ls -t dist/build-* | head -1 | xargs basename)
    
    echo "Build ID: $BUILD_ID"
    
    # Parse JSONL for CI metrics
    jq -r '
      select(.level == "ERROR") | 
      "[ERROR] \(.stage): \(.message)"
    ' dist/$BUILD_ID/logs/build.jsonl || true
    
    # Extract timing data
    jq -r '
      select(.duration_ms != null) | 
      "\(.stage): \(.duration_ms / 1000)s"
    ' dist/$BUILD_ID/logs/build.jsonl | sort -t: -k2 -rn || true
    
    # Upload artifacts
    cp -r dist/$BUILD_ID/logs/ ./build-logs/
    
- uses: actions/upload-artifact@v3
  if: always()
  with:
    name: build-logs-${{ github.run_id }}
    path: build-logs/
```

## Telemetry Dashboard Data Source

```python
# telemetry_collector.py
import json
from pathlib import Path

def collect_build_metrics(build_dir: Path) -> dict:
    """Extract structured metrics from JSONL logs"""
    jsonl_file = build_dir / "logs" / "build.jsonl"
    
    metrics = {
        "build_id": build_dir.name,
        "stages": [],
        "errors": [],
        "subprocesses": [],
        "timing": {},
    }
    
    with open(jsonl_file) as f:
        for line in f:
            entry = json.loads(line.strip())
            
            if entry.get("duration_ms"):
                stage = entry["stage"]
                metrics["timing"][stage] = entry["duration_ms"]
            
            if entry["level"] == "ERROR":
                metrics["errors"].append({
                    "stage": entry["stage"],
                    "message": entry["message"],
                    "timestamp": entry["timestamp"],
                })
            
            if entry.get("command"):
                metrics["subprocesses"].append({
                    "command": entry["command"],
                    "exit_code": entry.get("exit_code"),
                    "duration_ms": entry.get("duration_ms"),
                })
    
    return metrics

# Usage
metrics = collect_build_metrics(Path("./output/build-20260527-a1b2c3d4"))
print(json.dumps(metrics, indent=2))
```
