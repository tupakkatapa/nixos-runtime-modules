{ rustPlatform
, pkgs
, lib
, nix
}:
let
  manifest = (lib.importTOML ./Cargo.toml).package;
in
rustPlatform.buildRustPackage rec {
  pname = manifest.name;
  inherit (manifest) version;

  nativeBuildInputs = with pkgs; [
    pkg-config
    makeWrapper
  ];

  src = lib.sourceByRegex ./. [
    "^Cargo.toml$"
    "^Cargo.lock$"
    "^src.*$"
    "^tests.*$"
  ];

  cargoLock.lockFile = ./Cargo.lock;

  postInstall = ''
    wrapProgram $out/bin/${pname} \
      --prefix PATH : ${lib.makeBinPath [ nix ]}
  '';

  # Tests require access to a /nix/ and a nix daemon; we run them at pre-commit instead
  doCheck = false;
}
