
# NixOS Runtime Modules

A modular system for dynamically enabling and disabling NixOS configurations at runtime with a simple CLI. This approach keeps your base system configuration small while allowing you to add heavier components on demand.

## Why Use Runtime Modules?

*"You get another computer when you know how to use one."*

- Enables a modular approach where your base configuration stays minimal and shareable. Additional modules can come from anywhere — local paths, other flake inputs, public or private.

- Significantly reduces initial compilation time by only building what you need when you need it. While you could theoretically achieve similar results by manually commenting out imports in your configuration files, that approach quickly becomes tedious and breaks the integrity of your configuration relative to upstream.

- When working with embedded systems, bootable USB drives, or any environment where kernel/initrd size matters, runtime modules can provide the flexibility to keep your base system lean while keeping the functionality when needed.

### Why not use Specialisations?

While [NixOS specialisations](https://nixos.wiki/wiki/Specialisation) might seem like the perfect solution, they don't actually reduce your base image size. The key issue is that NixOS prepares the initrd to potentially boot into any of your specialisations without requiring a rebuild, so it must include all the components.

## How It Works

The system creates a temporary flake extending your base configuration with the specified modules. It maintains a runtime state in `/run/runtime-modules/` that tracks which modules are active. When you enable or disable modules, it updates this state and applies the changes using `nixos-rebuild test`.

### Limitations

Since this uses `nixos-rebuild test`, the running kernel is not replaced; that would require a power cycle or kexec. Therefore, changes that touch kernel modules or parameters do not take effect. If a module needs kernel-level support (e.g. libvirt requires KVM), that support must be present in the base system.

## Documentation

See the [documentation](./docs/runtime-modules.md).

