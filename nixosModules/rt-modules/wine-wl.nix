# Wine for Wayland
{ pkgs, ... }: {
  environment.systemPackages = with pkgs; [
    wineWow64Packages.waylandFull
    winetricks
  ];
}
