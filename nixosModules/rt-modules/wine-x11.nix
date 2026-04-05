# Wine for X11
{ pkgs, ... }: {
  environment.systemPackages = with pkgs; [
    wineWowPackages.full
    winetricks
  ];
}
