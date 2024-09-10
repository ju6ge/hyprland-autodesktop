Sway Autodesktop
====================

This tool is a re-implementation of `kanshi` for `Sway`. But with some extra features:

- Dynamic Workspace assignment (sway only)
- Monitor Physical Input Detection
- wlroots compatible

This program needs to run as a daemon to listen to wayland wlr protocol event to detect when monitors are attached or detached from the device. This information is used to apply a configuration profile ordering displays in a specific way. External scripts can be used to apply a specific network profile or power-profile. It will also detect which physical input a monitor is currently using (via ddc) making profile selection more powerful.

Running the program with a command will communicate with the daemon process to get state information or force a specific profile.

### Commands
- `sway-autodesktop pid`
- `sway-autodesktop current-profile`
- `sway-autodesktop profiles`
- `sway-autodesktop apply <profile>`
- `sway-autodesktop attached`
- `sway-autodesktop monitor-inputs`

### Configuration 

workplaces.yml
``` yaml
profiles:                                                       # named profiles to try to detect when monitors are attached and dettached
  laptop:                                                       # base profile for laptops with a single built in display
    screens:
    - identifier: eDP-1                                         # screen identifier
      scale: 1.0
      rotation: Landscape
      wallpaper: /tmp/test.png
      position: Root
      enabled: true
    scripts:
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
    scripts:
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
    scripts:
    - sudo systemctl stop iwd                                   # disable wifi
    - /usr/bin/powerprofilesctl set performance                 # set device powerprofile
```


