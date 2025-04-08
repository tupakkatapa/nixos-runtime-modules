{
  nixConfig = {
    extra-substituters = [
      "https://cache.nixos.org"
      "https://nix-community.cachix.org"
    ];
    extra-trusted-public-keys = [
      "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY="
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };

  inputs = {
    devenv.url = "github:cachix/devenv";
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";
    treefmt-nix.url = "github:numtide/treefmt-nix";
  };

  outputs = { self, ... }@inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = inputs.nixpkgs.lib.systems.flakeExposed;
      imports = [
        inputs.devenv.flakeModule
        inputs.treefmt-nix.flakeModule
      ];

      perSystem =
        { pkgs
        , config
        , ...
        }:
        {
          # Nix code formatter -> 'nix fmt'
          treefmt.config = {
            projectRootFile = "flake.nix";
            flakeFormatter = true;
            flakeCheck = true;
            programs = {
              nixpkgs-fmt.enable = true;
              deadnix.enable = true;
              statix.enable = true;
              rustfmt.enable = true;
            };
          };

          # Development shell -> 'nix develop' or 'direnv allow'
          devenv.shells = {
            default = {
              packages = with pkgs; [
                cargo-tarpaulin
              ];
              languages.rust = {
                enable = true;
                components = [ "cargo" "clippy" ];
              };
              pre-commit.hooks = {
                treefmt = {
                  enable = true;
                  package = config.treefmt.build.wrapper;
                };
                pedantic-clippy = {
                  enable = true;
                  entry = "cargo clippy -- -D clippy::pedantic";
                  files = "\\.rs$";
                  pass_filenames = false;
                };
                cargo-test = {
                  enable = true;
                  entry = "cargo test --all-features";
                  files = "\\.rs$";
                  pass_filenames = false;
                };
              };
              # Workaround for https://github.com/cachix/devenv/issues/760
              containers = pkgs.lib.mkForce { };
            };
          };
        };

      flake =
        {
          nixosModules = {
            runtimeModules.imports = [ ./nixosModules ];
            default = self.nixosModules.runtimeModules;
          };
        };
    };
}
