# SideGlow

[![CI](https://github.com/MangriMen/side-glow/actions/workflows/ci.yml/badge.svg)](https://github.com/MangriMen/side-glow/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/MangriMen/side-glow)](https://github.com/MangriMen/side-glow/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Ambient light for multi-monitor setups on Windows. SideGlow captures the edges of your
main screen and lets the neighbouring monitors glow in the matching colors, like an
Ambilight TV.

<!-- TODO: add a screenshot or GIF of the glow in action. -->

## Download

Grab `SideGlow.exe` from the [latest release](https://github.com/MangriMen/side-glow/releases/latest)
and run it. It is a single portable executable: no installer, nothing to unpack.

To verify the download, compare it against the published checksum:

```powershell
(Get-FileHash .\SideGlow.exe -Algorithm SHA256).Hash
Get-Content .\SideGlow.exe.sha256
```

## Features

- Any monitor layout: monitors on the left, right, above or below, different resolutions,
  vertical offsets, portrait orientation and mixed DPI scaling.
- Automatic layout: every monitor that touches a captured monitor gets a glow. Click
  monitors in the settings map to choose which ones are captured.
- Several colors per edge (zones), with stretched or physically aligned mapping.
- Physically based glow: brightness falls off like light from a real LED strip, so bright
  zones reach further than dim ones.
- Per-output mode: **Overlay** (transparent, click-through, on top of your windows) or
  **Dedicated** (the monitor is only used as a light and fades into black).
- Global zone, depth and opacity settings, with per-output overrides.
- Frame-rate independent smoothing, linear-light color averaging, dithered gradients.
- Low overhead: only the edge zones are read back from the GPU, and nothing is redrawn
  while the screen doesn't change.

## Usage

SideGlow lives in the system tray. Left-click the icon (or pick **Settings**) to open the
settings window. The tray menu can also pause the glow or quit. Hover any setting for a
short explanation.

Settings are saved automatically to `%APPDATA%\SideGlow\config.toml`, which can also be
edited by hand while SideGlow is not running. Release builds write a log to
`%APPDATA%\SideGlow\sideglow.log`; set `SIDEGLOW_LOG=debug` for more detail.

The glow windows are excluded from screen capture, so they don't show up in screenshots
or recordings (and don't feed back into the capture).

## Requirements

Windows 10 version 2004 or newer (Windows Graphics Capture). On builds before Windows 11
24H2 the capture rate is limited in software instead of by the OS.

## Building from source

You need a recent stable Rust toolchain with the MSVC target.

```sh
cargo build --release   # target/release/SideGlow.exe
cargo test
```

### Benchmark

`examples/test_pattern.rs` opens a full-screen static, slow or fast (video-like) pattern on
the primary monitor, and `scripts/bench.ps1` measures the CPU and GPU load of several
SideGlow builds against it. Leave the PC idle while it runs.

```sh
cargo build --release --examples
```

```powershell
./scripts/bench.ps1 -Builds ([ordered]@{ old = 'path\to\old.exe'; new = 'target/release/SideGlow.exe' }) -Rounds 2
```

## Architecture

| Module     | Responsibility                                                              |
|------------|-----------------------------------------------------------------------------|
| `config`   | Serializable settings and their TOML storage                                 |
| `display`  | Monitor enumeration and the pure layout geometry (adjacency, segments)       |
| `capture`  | One capture session per source monitor, reading back and averaging the zones |
| `glow`     | Color bus between capture threads and windows, smoothing, color math         |
| `ui`       | Glow and zone preview windows, the settings window and the tray icon         |

The root egui viewport is the settings window; it is usually hidden, but its `logic` pass
drives everything else. Glow and zone windows are immediate viewports rendered from that
pass; the color bus wakes it whenever capture publishes new colors, so nothing repaints
while the screen is static. (Deferred viewports are avoided on purpose: egui drops
repaint requests for unfocused ones, see egui
[#8466](https://github.com/emilk/egui/issues/8466) and
[#4945](https://github.com/emilk/egui/issues/4945).)

## Releasing

1. Update `version` in `Cargo.toml` (and `Cargo.lock`), and move the `[Unreleased]` entries
   of [CHANGELOG.md](CHANGELOG.md) under a new version heading.
2. Commit as `chore: release vX.Y.Z`, then tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. The release workflow builds `SideGlow.exe`, and publishes it with its SHA-256 checksum
   and the changelog entry as a GitHub release.

## License

[MIT](LICENSE)
