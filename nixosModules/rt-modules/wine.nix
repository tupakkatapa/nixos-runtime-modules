# Wine Windows compatibility layer
{ pkgs, ... }: {
  environment.systemPackages = with pkgs; [
    wineWowPackages.waylandFull
    winetricks
  ];
}
