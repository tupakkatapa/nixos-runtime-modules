# NixOS Runtime Modules

A modular system for dynamically enabling and disabling NixOS configurations at runtime. It allows you to keep your base system configuration small while dynamically enabling and disabling heavier components as needed. This is especially useful when you need to keep your initrd + kernel size under a specific limit.

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

## Configuration

### Configuration Options:

- **`enable`** – Enables the runtime modules system.
- **`flakeUrl`** – The base flake reference to extend from.
- **`modules`** – List of modules that can be dynamically enabled/disabled.

### Example Configuration:

```nix
{
  services.runtimeModules = {
    enable = true;
    flakeUrl = "github:<owner>/<repository>";

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

## Usage

The `runtime-module` command is available after enabling the module:

```bash
$ runtime-module --help
Usage: runtime-module <COMMAND>

Commands:
  enable   Build and enable one or more modules
  disable  Disable one or more specific modules
  reset    Disable all modules (revert to base system)
  status   Show module status (enabled/disabled)
  list     List all available modules

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## How It Works

The system creates a temporary flake extending your base configuration with the specified module, then applies it using `nixos-rebuild test`. This allows dynamic reconfiguration without rebuilding your entire system or exceeding size limits.
