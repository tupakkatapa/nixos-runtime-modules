# NixOS Runtime Modules

A modular system for dynamically enabling and disabling NixOS configurations at runtime. This approach keeps your base system configuration small while allowing you to add heavier components on demand.

## Getting Started

Add this repository as a Nix flake input, then enable the module in your NixOS configuration:

```nix
{
  inputs = {
    runtime-modules.url = "github:tupakkatapa/nixos-runtime-modules";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };
  outputs = { self, ... }@inputs: {
    nixosConfigurations = {
      yourhostname = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          ./configuration.nix
          inputs.runtime-modules.nixosModules.runtimeModules
          {
            # Module configuration
            services.runtimeModules = { ... };
          }
        ];
      };
    };
  };
}
```

## Configuration Options

- `enable`: Enable the runtime modules system.
- `flakeUrl`: Base flake reference to extend from (`path:`, `github:`, etc).
- `builtinModules.enable`: Enable upstream modules (prefixed `rt.`, auto-discovered from `rt-modules/`).

### Module Definition

Each entry in `modules`:

- `name`: Unique identifier used in CLI commands.
- `imports`: List of module imports (paths or parameterized imports). Replaces deprecated `path`.
- `desc`: Short description, shown in `runtime-modules list`.
- `postEnable`: Command to run after enabling. Runs in the background, detached from the parent process. Default: `null`.
- `postDisable`: Command to run after disabling. Runs in the background, detached from the parent process. Default: `null`.
- `skipValidation`: Skip `nix flake check` validation. Default: `false`.

### Example

```nix
{
  services.runtimeModules = {
    enable = true;
    flakeUrl = "github:<owner>/<repository>";
    builtinModules.enable = true;
    modules = [
      {
        name = "gaming";
        imports = [ ./gaming.nix ];
        postEnable = "steam";
      }
      {
        name = "virtualization";
        imports = [
          ./podman.nix
          ./libvirt.nix
        ];
      }
    ];
  };
}
```

## CLI

Commands that modify system state require root privileges. The CLI will automatically elevate via `sudo` if needed. Run `runtime-modules --help` for a quick overview.

### Commands

- `enable <modules...>`: Updates state, runs `nix flake update`, then `nixos-rebuild test`. Already-enabled modules are skipped; use `--force` to rebuild anyway. Runs `postEnable` hooks after successful rebuild.
- `disable <modules...>`: Same rebuild flow as `enable`. Runs `postDisable` hooks after success.
- `reset`: Disables all modules, reverts to base system. Skips rebuild if already at base state unless `--force`.
- `status <modules...>`: Prints `enabled`, `disabled`, or `uncertain` per module. Exits non-zero if any module is not fully enabled — useful for scripting.
- `list`: Lists all modules grouped into user and upstream sections. State markers: `[✓]` enabled, `[ ]` disabled, `[?]` uncertain.
- `rebuild`: Rebuilds with currently enabled modules without changing state. Use to recover from a failed rebuild.

### Options

- `--json`, `-j`: JSON output. `list` groups into `user_modules` and `upstream_modules` arrays; `status` returns module status objects.

  ```json
  {
    "user_modules": [
      {
        "name": "gaming",
        "path": "",
        "state": "Enabled",
        "desc": "Gaming setup",
        "postEnable": "steam",
        "postDisable": null
      }
    ],
    "upstream_modules": []
  }
  ```
- `--force`, `-f`: Force rebuild even when no state changes are detected.
- `--help`, `-h`: Print help.
- `--version`, `-V`: Print version.

## Module States

- **Enabled**: Active, rebuild succeeded.
- **Disabled**: Not active.
- **Uncertain**: Rebuild failed mid-way. Resolve with `rebuild` or `reset`.

State lives in `/run/runtime-modules/` (tmpfs). Reboot reverts to base system.

## Hooks

The `postEnable` and `postDisable` hooks run after a successful enable or disable. They are detached from the parent process with their own process group and stdio redirected to `/dev/null`.

When the CLI is run via `sudo`, hooks drop privileges back to the invoking user via `runuser` with `XDG_RUNTIME_DIR` set for session access. When already running as root without `SUDO_USER` (e.g. from a systemd service), hooks run as root directly.

## Validation

During `nix flake check`, each module is evaluated individually in a minimal NixOS context to catch configuration errors early. Custom `specialArgs` from your base configuration are available during validation. Set `skipValidation = true` for modules that depend on options defined by other modules or external state.

## Built-in Modules

Available when `builtinModules.enable = true`. All prefixed with `rt.`.

| Module | Description |
|--------|-------------|
| `rt.docker` | Docker containers with compose support |
| `rt.gimp` | GIMP image editor with plugins |
| `rt.kdenlive` | Kdenlive video editor with plugins |
| `rt.libreoffice` | LibreOffice with spell-checking |
| `rt.obs-studio` | OBS Studio with common plugins |
| `rt.podman` | Podman containers with compose support |
| `rt.steam` | Steam with GameMode performance optimization |
| `rt.wine-wl` | Wine for Wayland |
| `rt.wine-x11` | Wine for X11 |

## Troubleshooting

- **"one or more modules not found"**: Name mismatch. Check `runtime-modules list`.
- **Stuck in uncertain state**: Previous rebuild failed, module might still be functional. Run `sudo runtime-modules rebuild` or `reset` to resolve.
- **Hooks not running**: Only run after successful enable/disable. Hook output is discarded; test manually first.
- **State reset on reboot**: By design. `/run` is tmpfs.
