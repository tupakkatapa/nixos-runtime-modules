# OBS Studio with common plugins
{ pkgs, ... }: {
  programs.obs-studio = {
    enable = true;
    plugins = with pkgs.obs-studio-plugins; [
      wlrobs
    ];
  };
}
