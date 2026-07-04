# Steam with GameMode performance optimization
{ pkgs, lib, ... }: {
  # https://github.com/nixos/nixpkgs/issues/523200
  security.wrappers.bwrap = lib.mkForce {
    source = "${pkgs.bubblewrap}/bin/bwrap";
    owner = "root";
    group = "root";
    setuid = false;
    setgid = false;
  };

  programs.steam = {
    enable = true;
    gamescopeSession = {
      enable = true;
      args = [
        "--mangoapp"
        "--rt"
        "--steam"
        # "--hdr-enabled"
      ];
      env = {
        MANGOHUD = "1";
        MANGOHUD_CONFIG = "full";
      };
    };
    protontricks.enable = true;
    localNetworkGameTransfers.openFirewall = true;
  };
  programs.gamescope = {
    enable = true;
    capSysNice = true;
  };

  programs.gamemode = {
    enable = true;
    settings = {
      general = { renice = 10; };
      gpu = {
        apply_gpu_optimisations = "accept-responsibility";
        gpu_device = 0;
        amd_performance_level = "high";
        nv_powermizer_mode = 1;
      };
      custom = {
        start = "${pkgs.libnotify}/bin/notify-send 'GameMode started'";
        end = "${pkgs.libnotify}/bin/notify-send 'GameMode ended'";
      };
    };
  };

  environment.systemPackages = with pkgs; [
    mangohud
  ];
}
