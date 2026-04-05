# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] - 2026-04-05

### Added
- `postEnable` and `postDisable` hooks for running commands after enabling/disabling modules
- Documentation in `docs/`
- Built-in modules: `rt.docker`, `rt.obs-studio`, `rt.wine-x11`

### Changed
- Help message and stdout formatting
- Renamed `rt.wine` to `rt.wine-wl`
- Improved `rt.steam` with MangoHUD, gamescope session args, and LAN game transfers

## [0.2.0] - 2026-02-14

### Added
- Module validation during `nix flake check` and `skipValidation` option
- `imports` option, replacing deprecated `path`
- State-based architecture (Enabled/Disabled/Uncertain)

### Changed
- Binary renamed from `runtime-module` to `runtime-modules`
- Built-in modules auto-discovered from `rt-modules/` directory

## [0.1.0] - 2025-03-22

### Added
- Initial effort
