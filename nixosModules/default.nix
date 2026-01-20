# nixosModules/runtime-modules.nix
#
# Note: options and modulesPath are listed in args to ensure they're filtered
# out when extracting custom specialArgs for module validation.
{ config, lib, pkgs, ... }@args:
let
  cfg = config.services.runtimeModules;
  dataDir = "/run/runtime-modules";
  stateJson = "${dataDir}/state.json";

  # Extract custom specialArgs by filtering out standard NixOS module args
  standardArgs = [ "config" "lib" "pkgs" "options" "modulesPath" ];
  inheritedSpecialArgs = lib.filterAttrs (n: _: !(builtins.elem n standardArgs)) args;

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
          imports = [ (libDir + "/${file}") ];
          desc = getDescription file;
        })
        nixFiles
    else
      [ ];

  # Normalize module: append deprecated 'path' to 'imports'
  normalizeModule = module:
    if module.path != null then
      module // { imports = module.imports ++ [ module.path ]; }
    else
      module;

  # All modules = user modules (normalized) + upstream modules
  allModules = (map normalizeModule cfg.modules) ++ rtModules;

  # Check for deprecated 'path' attribute
  modulesWithPath = builtins.filter (m: m.path != null) cfg.modules;
  hasDeprecatedPath = modulesWithPath != [ ];

  # Minimal NixOS config for module validation
  minimalValidationConfig = {
    boot.loader.grub.enable = false;
    fileSystems."/" = { device = "/dev/null"; fsType = "ext4"; };
    system.stateVersion = config.system.stateVersion;
    nixpkgs.hostPlatform = pkgs.stdenv.hostPlatform.system;
    nixpkgs.config.allowUnfree = true;
  };

  # Evaluate each module in a minimal NixOS context
  validateModule = module:
    let
      eval = import (pkgs.path + "/nixos/lib/eval-config.nix") {
        inherit (pkgs.stdenv.hostPlatform) system;
        specialArgs = inheritedSpecialArgs;
        modules = module.imports ++ [ minimalValidationConfig ];
      };
    in
    # Force evaluation by accessing the derivation path
    builtins.seq eval.config.system.build.toplevel.drvPath true;

  # Generate the modules.json content
  modulesJson = builtins.toJSON {
    modules = map
      (module: {
        inherit (module) name desc;
        state = "Disabled";
      })
      allModules;
  };

  # Build the Rust program
  moduleManagerRust = pkgs.callPackage ../package.nix {
    inherit (pkgs) rustPlatform nix;
  };

  # Create a static flake file that dynamically resolves enabled modules
  staticFlakeFile = pkgs.writeTextFile {
    name = "runtime-modules-flake";
    destination = "/flake.nix";
    text = ''
      {
        description = "Runtime modules configuration";

        # Inherit nixpkgs from base flake
        inputs.base.url = "${cfg.flakeUrl}";
        inputs.nixpkgs.follows = "base/nixpkgs";

        outputs = { self, nixpkgs, base }:
          let
            # Read enabled modules from state file
            stateFile = ${dataDir}/state.json;
            state = builtins.fromJSON (builtins.readFile stateFile);
            enabledNames = state.enabled or [];

            # Get all module definitions from base config
            baseConfig = base.nixosConfigurations.${config.networking.hostName}.config;
            allModules = baseConfig.services.runtimeModules._allModules;

            # Filter to only enabled modules and collect their imports
            enabledModules = builtins.filter (m: builtins.elem m.name enabledNames) allModules;
            enabledImports = builtins.concatMap (m: m.imports) enabledModules;
          in
          {
            nixosConfigurations.runtime = base.nixosConfigurations.${config.networking.hostName}.extendModules {
              modules = enabledImports ++ [
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

    modules = lib.mkOption {
      type = lib.types.listOf (lib.types.submodule {
        options = {
          name = lib.mkOption {
            type = lib.types.str;
            description = "Name of the module";
          };

          imports = lib.mkOption {
            type = lib.types.listOf lib.types.unspecified;
            default = [ ];
            description = "List of module imports";
          };

          path = lib.mkOption {
            type = lib.types.nullOr lib.types.path;
            default = null;
            description = "Deprecated: use 'imports' instead";
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

    _allModules = lib.mkOption {
      type = lib.types.listOf lib.types.unspecified;
      internal = true;
      readOnly = true;
      description = "All modules including builtins";
    };
  };

  config = lib.mkIf cfg.enable {
    services.runtimeModules._allModules = allModules;

    environment.systemPackages = [
      moduleManagerRust
    ];

    # Validate all modules
    assertions = [
      {
        assertion = !hasDeprecatedPath;
        message = ''
          services.runtimeModules: 'path' option has been replaced by 'imports'.
          Affected modules: ${lib.concatMapStringsSep ", " (m: m.name) modulesWithPath}

          Migration: change 'path = ./file.nix;' to 'imports = [ ./file.nix ];'

          Example:
            { name = "foo"; path = ./foo.nix; }
          becomes:
            { name = "foo"; imports = [ ./foo.nix ]; }
        '';
      }
      {
        assertion = lib.all validateModule allModules;
        message = "";
      }
    ];

    # Ensure the directory exists during activation
    system.activationScripts.runtimeModulesSetup = lib.stringAfter [ "etc" "users" "groups" ] ''
      echo "[runtime-modules] setting up ${dataDir}..."
      mkdir -p -m 755 ${dataDir}

      # Copy the static flake file
      cp -f ${staticFlakeFile}/flake.nix ${dataDir}/flake.nix
      chmod 644 ${dataDir}/flake.nix

      # Write the modules.json file
      echo '${modulesJson}' > ${dataDir}/modules.json
      chmod 644 ${dataDir}/modules.json

      # Auto-reset state if not running on a runtime-modules system
      if [ ! -f "/etc/runtime-modules-enabled" ]; then
        if [ -f "${stateJson}" ]; then
          echo "[runtime-modules] standard system detected, cleaning runtime state..."
          rm ${stateJson}
        fi
      fi
    '';
  };
}

