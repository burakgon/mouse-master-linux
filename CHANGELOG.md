# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-10-20

### Added

- First release of Mouse Master: the `mouse-masterd` service, the Qt 6 app and
  `mouse-masterctl`.
- Button remapping with built-in actions, keyboard shortcuts, mouse buttons, commands,
  applications, pointer speed and Easy-Switch.
- Mouse gestures on any capable button, with presets, repeat and pan mode.
- Per-application profiles with automatic switching on KDE Plasma, Hyprland and Sway.
- Pointer speed with presets and a precision mode; SmartShift, wheel mode, torque, smooth
  and natural scrolling.
- Thumb wheel modes (scroll speed, zoom, volume, brightness, tabs, desktops, undo/redo,
  custom) and thumb wheel tap.
- Easy-Switch host names, renaming and switching; battery notifications; haptics.
- Bluetooth, Bolt, Unifying and Lightspeed support.
- A HID++ simulator, an end-to-end test suite and `mouse-masterd --demo`.
- `mouse-masterctl doctor`, and a warning in the app while Solaar or logiops is running.
- Smooth zoom like a touchpad pinch, from the thumb wheel or a zoom gesture mode.
- The app offers a restart when it was updated while open, so an old window cannot
  save settings it does not know.

[Unreleased]: https://github.com/burakgon/mouse-master-linux/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/burakgon/mouse-master-linux/releases/tag/v0.1.0
