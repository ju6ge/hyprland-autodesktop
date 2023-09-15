Hyprland Workplaces Profiles
============================

This tool is a re-implementation of `kanshi`` for `Hyprland`. But with some extra features. 

This program needs to run as a daemon to listen to wayland wlr protocol event to detect when monitors are attached or detached from the device. This information is used to apply a configuration profile ordering displays in a specific way. External skripts can be used to apply a specific network profile or power-profile. It will also detect which physical input a monitor is currently using (via ddc) making profile selection more powerful.

Running the program with a command will communicate with the daemon process to get state information or force a specific profile.

### Commands
TODO!

### Configuration 

workplaces.yml
``` yaml
hyprland_config_file: "/home/judge/.config/hypr/monitor.conf"   # where to put the hyprland config (sould be sourced from the main hyprland config file)
profiles:                                                       # named profiles to try to detect when monitors are attached and dettached
  laptop:
    screens:
    - identifier: eDP-1                                         # screen identifier
      scale: 1.0
      rotation: Landscape
      display_output_code: null                                 # monitor input
      wallpaper: /tmp/test.png
      position: Root
      enabled: true
    skripts:
    - sudo systemctl start iwd
    - sudo /etc/systemd/network/default.sh
    - /usr/bin/powerprofilesctl set power-saver
```


