# nixosModules/runtime-modules.nix
{ config, lib, pkgs, ... }:
let
  cfg = config.services.runtimeModules;
  dataDir = "/run/runtime-modules";

  # Generate the modules.json content
  modulesJson = builtins.toJSON {
    modules = map
      (module: {
        inherit (module) name;
        path = toString module.path;
      })
      cfg.modules;
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
              ./runtime-modules.nix
              # Add a marker file to detect systems built using runtime-modules
              { environment.etc."runtime-modules-enabled".text = "true"; }
            ];
          };
        };
      }
    '';
  };

  # Create the module manager script by substituting values directly
  moduleManagerScript =
    let
      # Read the main script content
      scriptContent = builtins.readFile ./main.sh;

      # Substitute configuration values directly into the script
      scriptWithValues = builtins.replaceStrings
        [
          "@DATA_DIR@"
          "@MODULES_JSON@"
        ]
        [
          "${dataDir}"
          "${modulesJson}"
        ]
        scriptContent;
    in
    pkgs.writeShellApplication {
      name = "runtime-module";
      runtimeInputs = with pkgs; [
        coreutils
        gnugrep
        jq
        nix
      ];
      text = scriptWithValues;
    };

in
{
  options.services.runtimeModules = {
    enable = lib.mkEnableOption "NixOS runtime modules system";

    flakeUrl = lib.mkOption {
      type = lib.types.str;
      description = "Base flake reference to extend from";
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
        };
      });
      default = [ ];
      description = "Runtime modules definition";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [
      moduleManagerScript
    ];

    # Ensure the directory exists during activation
    system.activationScripts.runtimeModulesSetup = lib.stringAfter [ "etc" "users" "groups" ] ''
      echo "[runtime-modules] setting up ${dataDir}..."
      mkdir -p -m 644 ${dataDir}

      # Copy the static flake file
      cp -f ${staticFlakeFile}/flake.nix ${dataDir}/flake.nix
      chmod 644 ${dataDir}/flake.nix

      # Auto-reset state if not running on a runtime-modules system
      if [ ! -f "/etc/runtime-modules-enabled" ]; then
        if [ -f "${dataDir}/runtime-modules.nix" ]; then
          [runtime-modules] standard system detected, cleaning runtime state...
          rm ${dataDir}/runtime-modules.nix
        fi
      fi
    '';
  };
}
