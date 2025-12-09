# nixosModules/runtime-modules.nix
{ config, lib, pkgs, ... }:
let
  cfg = config.services.runtimeModules;
  dataDir = "/run/runtime-modules";
  modulesNix = "${dataDir}/runtime-modules.nix";

  # Upstream modules
  libDir = ./rt-modules;
  rtModules =
    if (cfg.builtinModules.enable && builtins.pathExists libDir) then
      let
        files = builtins.attrNames (builtins.readDir libDir);
        nixFiles = builtins.filter (f: lib.hasSuffix ".nix" f) files;

        # Simple function to extract description from first line
        getDescription = file:
          let
            content = builtins.readFile (libDir + "/${file}");
            firstLine = lib.head (lib.splitString "\n" content);
          in
          lib.strings.trim (lib.removePrefix "#" firstLine);
      in
      map
        (file: {
          name = "rt." + (lib.removeSuffix ".nix" file);
          path = libDir + "/${file}";
          desc = getDescription file;
        })
        nixFiles
    else
      [ ];

  # All modules = user modules + upstream modules
  allModules = cfg.modules ++ rtModules;

  # Generate the modules.json content
  modulesJson = builtins.toJSON {
    modules = map
      (module: {
        inherit (module) name path desc;
        state = "Disabled";
      })
      allModules;
  };

  # Build the Rust program
  moduleManagerRust = pkgs.callPackage ../package.nix {
    inherit (pkgs) rustPlatform nix;
  };

  # Create a static flake file that imports a dynamically generated modules file
  staticFlakeFile = pkgs.writeTextFile {
    name = "runtime-modules-flake";
    destination = "/flake.nix";
    text = ''
      {
        description = "Runtime modules configuration";

        # Inherit nixpkgs from base flake
        inputs.base.url = "${cfg.flakeUrl}";
        inputs.nixpkgs.follows = "base/nixpkgs";

        outputs = { self, nixpkgs, base }: {
          nixosConfigurations.runtime = base.nixosConfigurations.${config.networking.hostName}.extendModules {
            modules = [
              # Import the dynamically generated modules file
              ${modulesNix}
              # Add a marker file to detect systems built using runtime-modules
              { environment.etc."runtime-modules-enabled".text = "true"; }
            ];
          };
        };
      }
    '';
  };
in
{
  options.services.runtimeModules = {
    enable = lib.mkEnableOption "NixOS runtime modules system";

    flakeUrl = lib.mkOption {
      type = lib.types.str;
      description = "Base flake reference to extend from";
    };

    builtinModules.enable = lib.mkEnableOption "Enable built-in module library";

    specialArgs = lib.mkOption {
      type = lib.types.attrsOf lib.types.anything;
      default = config._module.args;
      defaultText = lib.literalExpression "config._module.args";
      description = ''
        An attribute set of extra arguments to be passed to the module functions.
        Defaults to the parent configuration's module arguments, ensuring consistency
        between validation and runtime.
      '';
    };

    modules = lib.mkOption {
      type = lib.types.listOf (lib.types.submodule {
        options = {
          name = lib.mkOption {
            type = lib.types.str;
            description = "Name of the module";
          };

          path = lib.mkOption {
            type = lib.types.path;
            description = "Path to the Nix file containing the module configuration";
          };

          desc = lib.mkOption {
            type = lib.types.str;
            default = "";
            description = "Description of what the module provides";
          };
        };
      });
      default = [ ];
      description = "Runtime modules definition";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [
      moduleManagerRust
    ];

    # Validate runtime modules at build time by evaluating them in a NixOS context
    assertions = map
      (m:
        let
          eval = lib.evalModules {
            inherit (cfg) specialArgs;
            modules = (import (pkgs.path + "/nixos/modules/module-list.nix")) ++ [
              m.path
              {
                nixpkgs.hostPlatform = lib.mkDefault pkgs.system;
                fileSystems."/" = lib.mkDefault { device = "none"; fsType = "tmpfs"; };
                boot.loader.grub.enable = lib.mkDefault false;
                system.stateVersion = lib.mkDefault (config.system.stateVersion or "24.11");
              }
            ];
          };
        in
        {
          assertion = eval.config != null;
          message = "Runtime module '${m.name}' at ${toString m.path} failed validation";
        }
      )
      allModules;

    # Ensure the directory exists during activation
    system.activationScripts.runtimeModulesSetup = lib.stringAfter [ "etc" "users" "groups" ] ''
      echo "[runtime-modules] setting up ${dataDir}..."
      mkdir -p -m 755 ${dataDir}

      # Copy the static flake file
      cp -f ${staticFlakeFile}/flake.nix ${dataDir}/flake.nix
      chmod 644 ${dataDir}/flake.nix

      # Write the modules.json file
      echo '${modulesJson}' > ${dataDir}/modules.json
      chmod 755 ${dataDir}/modules.json

      # Auto-reset state if not running on a runtime-modules system
      if [ ! -f "/etc/runtime-modules-enabled" ]; then
        if [ -f "${modulesNix}" ]; then
          echo "[runtime-modules] standard system detected, cleaning runtime state..."
          rm ${modulesNix}
        fi
      fi
    '';
  };
}

