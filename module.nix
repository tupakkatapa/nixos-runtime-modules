# nixosModules/runtime-modules.nix
{ config, lib, pkgs, ... }:
let
  cfg = config.services.runtimeModules;

  # Create the module manager script by substituting values directly
  moduleManagerScript =
    let
      # Read the main script content
      scriptContent = builtins.readFile ./main.sh;

      # Substitute configuration values directly into the script
      scriptWithValues = builtins.replaceStrings
        [
          "@DATA_DIR@"
          "@HOST_NAME@"
          "@FLAKE_URL@"
        ]
        [
          "${cfg.dataDir}"
          "${config.networking.hostName}"
          "${cfg.flakeUrl}"
        ]
        scriptContent;
    in
    pkgs.writeShellApplication {
      name = "runtime-module";
      runtimeInputs = with pkgs; [
        nix
        coreutils
        gnugrep
        git
        rsync
        findutils
        gnused
        gawk
        jq
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

    dataDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/runtime-modules";
      description = "Directory to store module files";
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

    # Set up the runtime modules environment
    system.activationScripts.runtimeModulesSetup = ''
            echo "Setting up runtime modules in ${cfg.dataDir}..."
            mkdir -p ${cfg.dataDir}

            # Create state file to track enabled modules if it doesn't exist
            if [ ! -f ${cfg.dataDir}/state.json ]; then
              echo '{"enabledModules": []}' > ${cfg.dataDir}/state.json
            fi

            # Set up the module registry
            echo "Setting up module registry..."
            cat > ${cfg.dataDir}/modules.json << EOF
            {
              "modules": [
                ${lib.concatMapStringsSep ",\n          " (module: ''
                  {
                    "name": "${module.name}",
                    "path": "${toString module.path}"
                  }
                '') cfg.modules}
              ]
            }
      EOF

            # Create needed directories if they don't exist
            mkdir -p ${cfg.dataDir}/original_flake

            # Ensure permissions are correct
            chown -R root:root ${cfg.dataDir}
            chmod -R 755 ${cfg.dataDir}
    '';
  };
}
