# Windows compatibility with gaming tools for Wayland
{ pkgs, ... }: {
  environment.systemPackages = with pkgs; [
    bottles
    lutris
    wineWowPackages.waylandFull
    winetricks
  ];
}


