Hyprland Autodesktop
====================

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
  laptop:                                                       # base profile for laptops with a single built in display
    screens:
    - identifier: eDP-1                                         # screen identifier
      scale: 1.0
      rotation: Landscape
      wallpaper: /tmp/test.png
      position: Root
      enabled: true
    skripts:
    - sudo systemctl start iwd                                  # enable wife (sudo scripts need to be explicitly whitelisted in visudo to work here)
    - /usr/bin/powerprofilesctl set power-saver                 # set device powerprofile
  docked_only_laptop:                                           # profile for docked mode but monitor input not set to dockingstation (maybe there is a worksation)
    screens:
    - identifier: Dell XXXXXXA                                  # screen identifier for specific dell monitor with serial
      scale: 1.0
      rotation: Landscape
      display_output_code: Any                                  # monitor input may be set to any input for this profile to match
      wallpaper: /tmp/test.png
      position: !RightOver eDP-1
      enabled: false
    - identifier: eDP-1                                         # build in laptop display
      scale: 1.0
      rotation: Landscape
      wallpaper: /tmp/test.png
      position: Root
      enabled: true
    skripts:
    - sudo systemctl start iwd                                  # disable wifi
    - /usr/bin/powerprofilesctl set power-saver                 # set device powerprofile
  docked_with_laptop:                                           # profile for docked with monitor as second screen enabled
    screens:
    - identifier: Dell XXXXXXA                                  # screen identifier for specific dell monitor with serial
      scale: 1.0
      rotation: Landscape
      display_output_code: Hdmi1                                # profile will only match if monitor is set to diplay Hdmi1 input
      wallpaper: /tmp/test.png
      position: Root
      enabled: true
    - identifier: eDP-1                                         # build in laptop display
      scale: 1.0
      rotation: Landscape
      wallpaper: /tmp/test.png
      position: !LeftUnder Dell XXXXXXA
      enabled: true
    skripts:
    - sudo systemctl stop iwd                                   # disable wifi
    - /usr/bin/powerprofilesctl set performance                 # set device powerprofile
```


