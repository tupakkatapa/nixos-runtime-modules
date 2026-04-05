# Kdenlive video editor with plugins
{ pkgs, ... }: {
  environment.systemPackages = with pkgs; [
    kdePackages.kdenlive
    ffmpeg
    frei0r
  ];
}
