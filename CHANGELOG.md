# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- Module state is read from the running system, which now records its modules in `/etc/runtime-modules-enabled`
- `enable`, `disable` and `reset` rebuild whenever the running system differs from the requested modules
- Clearer messages for failed rebuilds and for modules in an uncertain state

### Fixed
- Interrupted rebuilds no longer leave modules reported as enabled
- Rebuilds that fail after activation no longer leave applied modules uncertain
- Concurrent runs no longer race; operations wait for each other
- `state.json` is written atomically
- Module descriptions containing `'` no longer break activation
- Requested modules that no longer exist are dropped instead of forcing a rebuild on every run

## [0.3.0] - 2026-10-27

### Changed
- Bumped nixpkgs to `nixos-26.05`
- `rt.wine-wl` and `rt.wine-x11` now use `wineWow64Packages` (`wineWowPackages` deprecated upstream)

### Fixed
- Module validation now inherits host overlays and nixpkgs config, fixing missing overlay packages during `nix flake check`
- `rt.steam` bubblewrap regression ([nixpkgs#523200](https://github.com/nixos/nixpkgs/issues/523200))
- Skip empty `postEnable`/`postDisable` hooks (no more `executing ''`)

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
