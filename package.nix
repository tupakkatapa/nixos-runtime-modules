{ pkgs
, lib
, dataDir
, modulesJson
, modulesNix
, ...
}:
let
  packageName = "runtime-module";
in
pkgs.stdenv.mkDerivation rec {
  name = packageName;
  version = "0.1.0";

  src = ./.;

  buildInputs = with pkgs; [
    coreutils
    gnugrep
    jq
    nix
  ];

  nativeBuildInputs = [ pkgs.makeWrapper ];

  installPhase = ''
    mkdir -p $out/bin
    cp $src/main.sh $out/bin/${packageName}
    chmod +x $out/bin/${packageName}

    substituteInPlace $out/bin/${packageName} \
      --replace "@DATA_DIR@" "${dataDir}" \
      --replace "@MODULES_JSON@" '${modulesJson}' \
      --replace "@MODULES_NIX@" '${modulesNix}'

    wrapProgram $out/bin/${packageName} \
      --prefix PATH : ${lib.makeBinPath buildInputs}
  '';
}
