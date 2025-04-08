# DAWs and plugins with low-latency setup
{ pkgs, ... }: {
  environment.systemPackages = with pkgs; [
    ardour
    audacity
    drumgizmo
    guitarix
    gxplugins-lv2
    ladspaPlugins
    neural-amp-modeler-lv2
    qjackctl
    reaper
    tuxguitar
  ];

  # Make pipewire realtime-capable
  security.rtkit.enable = true;

  # Low-latency settings for audio production
  services.pipewire = {
    alsa.enable = true;
    alsa.support32Bit = true;
    pulse.enable = true;
    jack.enable = true;

    extraConfig = {
      pipewire."92-low-latency".context = {
        properties.default.clock = {
          rate = 48000;
          quantum = 64;
          min-quantum = 64;
          max-quantum = 64;
        };
      };
      pipewire-pulse."92-low-latency".context = {
        modules = [
          {
            name = "libpipewire-module-protocol-pulse";
            args = {
              pulse.min.req = "64/48000";
              pulse.default.req = "64/48000";
              pulse.max.req = "64/48000";
              pulse.min.quantum = "64/48000";
              pulse.max.quantum = "64/48000";
            };
          }
        ];
        stream.properties = {
          node.latency = "64/48000";
          resample.quality = 1;
        };
      };
    };
  };

  # User limits for real-time audio
  security.pam.loginLimits = [
    { domain = "@audio"; item = "rtprio"; type = "-"; value = "99"; }
    { domain = "@audio"; item = "memlock"; type = "-"; value = "unlimited"; }
    { domain = "@audio"; item = "nofile"; type = "soft"; value = "99999"; }
    { domain = "@audio"; item = "nofile"; type = "hard"; value = "99999"; }
  ];
}

