# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - TBD

First release.

### Added

- Ambient glow on neighbouring monitors from the edges of captured monitors, using
  Windows Graphics Capture.
- Support for any monitor layout: monitors on any side, different resolutions, vertical
  offsets, portrait orientation and mixed DPI scaling, with automatic adjacency detection.
- Settings window with a monitor map, a zones preview and hover help for every setting;
  tray icon with pause and quit.
- Overlay (transparent, click-through) and Dedicated (fades into black) output modes.
- Global zone count, capture depth and opacity settings with per-output overrides.
- Configurable capture frame rate.
- Physically based, uncapped glow falloff controlled by a spread setting.
- TOML config in `%APPDATA%\SideGlow\config.toml` with versioned migrations, and a log
  file in release builds.
- Repeatable benchmark: a full-screen test pattern example and `scripts/bench.ps1`.

### Fixed

- Glow windows no longer show Windows 11 rounded corners or the accent border, which hid
  a sliver along every edge.
- Glow and zone windows no longer randomly stop repainting when several are open
  (switched to immediate viewports).
- Mach banding next to bright objects: the glow mesh is subdivided and interpolated in
  linear light.

[Unreleased]: https://github.com/MangriMen/side-glow/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/MangriMen/side-glow/releases/tag/v0.1.0
