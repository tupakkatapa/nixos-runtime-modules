# Kdenlive video editor with plugins
{ pkgs, ... }: {
  environment.systemPackages = with pkgs; [
    kdePackages.kdenlive
    video-trimmer
    ffmpeg
    frei0r
  ];
}
