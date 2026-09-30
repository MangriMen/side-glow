# SideGlow

Ambient light for multi-monitor setups on Windows. SideGlow captures the edges of your
main screen and lets the neighbouring monitors glow in the matching colors, like an
Ambilight TV.

## Features

- Any monitor layout: monitors on the left, right, above or below, different resolutions,
  vertical offsets, portrait orientation and mixed DPI scaling.
- Automatic layout: every monitor that touches a captured monitor gets a glow. Click
  monitors in the settings map to choose which ones are captured.
- Several colors per edge (segments), with stretched or physically aligned mapping.
- Per-output mode: **Overlay** (transparent, click-through, on top of your windows) or
  **Dedicated** (the monitor is only used as a light and fades into black).
- Frame-rate independent smoothing, linear-light color averaging, dithered gradients.
- Low overhead: only the edge zones are read back from the GPU, and nothing is redrawn
  while the screen doesn't change.

## Usage

SideGlow lives in the system tray. Left-click the icon (or pick **Settings**) to open the
settings window. The tray menu can also pause the glow or quit.

Settings are saved automatically to `%APPDATA%\SideGlow\config.toml`, which can also be
edited by hand while SideGlow is not running. Release builds write a log to
`%APPDATA%\SideGlow\sideglow.log`; set `SIDEGLOW_LOG=debug` for more detail.

## Requirements

Windows 10 version 2004 or newer (Windows Graphics Capture). On builds before Windows 11
24H2 the capture rate is limited in software instead of by the OS.

## Building

```sh
cargo build --release
cargo test
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
drives everything else. Glow windows are deferred viewports that repaint independently,
only when the color bus publishes new colors for them.
