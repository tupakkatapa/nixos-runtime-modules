# GIMP image editor with plugins
{ pkgs, ... }: {
  environment.systemPackages = with pkgs; [
    gimp
    gimpPlugins.gmic
  ];
}
