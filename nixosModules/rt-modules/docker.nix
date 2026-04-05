# Docker containers with compose support
{ pkgs, ... }: {
  virtualisation.docker.enable = true;

  environment.systemPackages = with pkgs; [
    docker-compose
  ];
}
