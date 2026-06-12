# Wine for X11
{ pkgs, ... }: {
  environment.systemPackages = with pkgs; [
    wineWow64Packages.full
    winetricks
  ];
}
