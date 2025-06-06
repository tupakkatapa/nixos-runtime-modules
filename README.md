
# NixOS Runtime Modules

A modular system for dynamically enabling and disabling NixOS configurations at runtime with a simple CLI. This approach keeps your base system configuration small while allowing you to add heavier components on demand.

## Why Use Runtime Modules?

*"You get another computer when you know how to use one."*

- Significantly reduces initial compilation time by only building what you need when you need it. While you could theoretically achieve similar results by manually commenting out imports in your configuration files, that approach quickly becomes tedious and breaks the integrity of your configuration relative to upstream.

- When working with embedded systems, bootable USB drives, or any environment where kernel/initrd size matters, runtime modules can provide the flexibility to keep your base system lean while keeping the functionality when needed.

### Why not use Specialisations?

While [NixOS specialisations](https://nixos.wiki/wiki/Specialisation) might seem like the perfect solution, they don't actually reduce your base image size. The key issue is that NixOS prepares the initrd to potentially boot into any of your specialisations without requiring a rebuild, so it must include all the components.

## How It Works

The system creates a temporary flake extending your base configuration with the specified modules. It maintains a runtime state in `/run/runtime-modules/` that tracks which modules are active. When you enable or disable modules, it updates this state and applies the changes using `nixos-rebuild test`.

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

## Module Configuration

### Options

- **`enable`** – Enables the runtime modules system.
- **`flakeUrl`** - The base flake reference to extend from (should point to your system's configuration flake, using absolute paths with `path:` prefix for local flakes or other prefixes like `github:` for remote sources).
- **`builtinModules.enable`** - Enables the built-in module library (see section below).
- **`modules`** - List of modules that can be dynamically enabled/disabled. Each module has the following attributes:
  - **`name`** - The unique identifier for the module used in CLI commands.
  - **`path`** - Path to the Nix file containing the module configuration.
  - **`desc`** - An optional, short description of what the module provides.

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
        path = ./gaming.nix;
      }
      {
        name = "virtualization";
        path = ./virtualization.nix;
      }
    ];
  };
}
```

### Built-in Modules

The system includes a set of predefined, upstream modules that can be made available by enabling the `builtinModules.enable` option. Once enabled, you can manage these modules using the same `runtime-module` commands with their `rt.` prefix. Available upstream modules can be found in [rt-modules directory](./nixosModules/rt-modules).

## Usage

The `runtime-module` command is available after enabling the module:

```bash
$ runtime-module --help
Usage: runtime-module [OPTIONS] <COMMAND>

Commands:
  enable   Build and enable one or more modules
  disable  Disable one or more specific modules
  reset    Disable all modules (revert to base system)
  status   Show module status (enabled/disabled)
  list     List all available modules
  rebuild  Rebuild the system with currently enabled modules

Options:
  -j, --json     Output results in JSON format
  -f, --force    Force rebuild even if no changes are detected
  -h, --help     Print help
  -V, --version  Print version
```

### Examples

```bash
# Check all available modules and their status
runtime-module list

# Enable one or more modules
sudo runtime-module enable gaming virtualization

# Disable a module
sudo runtime-module disable gaming
```
