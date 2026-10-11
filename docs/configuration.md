# Configuration reference

Mouse Master keeps its settings in `~/.config/mouse-master/config.toml`. The app and
`mouse-masterctl` edit it for you; when editing by hand, stop the service first
(`systemctl --user stop mouse-master`) so your changes are not overwritten.

Hardware settings are optional: a setting that is not present is never touched, so the
device keeps whatever it had.

## General

```toml
[general]
notifications = true        # low battery notifications
low_battery_percent = 15    # warn below this level
osd = true                  # on screen display when DPI or wheel mode change from the mouse
app_profiles = true         # switch bindings with the focused application
```

## Devices

Each device has a section keyed by its id (`<product id>-<unit id>`, shown by
`mouse-masterctl list`). The id is the same whether the device is connected over Bluetooth
or a receiver.

```toml
[devices."b034-5C8F7AF8"]
name = "MX Master 3S"       # informational
```

### Settings

| Key | Type | Meaning |
| --- | --- | --- |
| `dpi` | integer | Pointer resolution. Snapped to what the sensor supports. |
| `dpi_presets` | list of integers | Values cycled through by `dpi-cycle`, `dpi-up` and `dpi-down` (2–8 values). |
| `wheel_mode` | `"ratchet"` or `"free-spin"` | Mode the wheel is put in when the device connects. |
| `smartshift` | boolean | Automatic switch to free spin when scrolling fast. |
| `smartshift_threshold` | 1–50 | Speed at which SmartShift switches; lower switches more easily. |
| `smartshift_torque` | 1–100 | Ratchet force, on devices with tunable torque (MX Master 4). |
| `hires_scroll` | boolean | High resolution ("smooth") wheel reports. |
| `invert_scroll` | boolean | Natural scrolling, done by the device. |
| `thumbwheel_invert` | boolean | Swap the thumb wheel's direction. |
| `haptics`, `haptics_level` | boolean, 0–100 | Haptic feedback and its intensity. |
| `report_rate_us` | integer | Report interval in microseconds (gaming mice), e.g. `1000` for 1 kHz. |
| `host_mode` | boolean | Gaming mice: use Mouse Master's settings instead of onboard profiles. |

### Bindings

`bindings.buttons` maps a control to an action. Control keys are `middle`, `back`,
`forward`, `gesture`, `mode-shift`, `scroll-left`, `scroll-right`, `dpi`, `haptic-panel`,
or `cid-XXXX` (hexadecimal control id) for anything else. A control without an entry
keeps its native function.

```toml
[devices."b034-5C8F7AF8".bindings.buttons]
back = { type = "builtin", id = "copy" }
forward = { type = "keys", keys = ["KEY_LEFTCTRL", "KEY_V"] }
middle = { type = "gestures", mode = "pan" }
```

The thumb wheel has its own entries:

```toml
[devices."b034-5C8F7AF8".bindings]
# Native horizontal scrolling when absent. Other choices:
thumbwheel = { type = "scroll", speed = 1.5 }
# thumbwheel = { type = "actions", left = { type = "builtin", id = "volume-down" }, right = { type = "builtin", id = "volume-up" }, step_degrees = 12 }
thumbwheel_tap = { type = "builtin", id = "play-pause" }   # touch sensitive wheels
```

### Application profiles

Profiles override bindings (and optionally the DPI) while one of their applications is
focused. Applications are matched case-insensitively against the window's app id, class or
desktop file id; `firefox` also matches `org.mozilla.firefox`.

```toml
[[devices."b034-5C8F7AF8".profiles]]
id = "p1"
name = "GIMP"
apps = ["gimp", "org.gimp.GIMP"]
enabled = true
dpi = 800
bindings.buttons.forward = { type = "keys", keys = ["KEY_LEFTCTRL", "KEY_Z"] }
bindings.thumbwheel = { type = "actions", left = { type = "keys", keys = ["KEY_LEFTBRACE"] }, right = { type = "keys", keys = ["KEY_RIGHTBRACE"] }, step_degrees = 10 }
```

## Actions

| `type` | Fields | Behaviour |
| --- | --- | --- |
| `default` | | The control's native function (the control is not diverted). |
| `none` | | Does nothing. |
| `builtin` | `id` | A built-in action, see below. |
| `keys` | `keys` | A key combination, held while the button is held. Names are Linux key codes such as `KEY_LEFTCTRL`, `KEY_F5` or `KEY_VOLUMEUP` (`KEY_` may be omitted). |
| `mouse` | `button` | `left`, `right`, `middle`, `back` or `forward`, held while the button is held. |
| `command` | `command` | A shell command, started in your session. |
| `open` | `target` | A URL, a file or folder (`~` is expanded), or an application's desktop file id. |
| `set-dpi` | `dpi` | Sets the pointer resolution and remembers it. |
| `shift-dpi` | `dpi` | Uses another resolution while the button is held (precision mode). |
| `switch-host` | `host` | Switches the device to another Easy-Switch channel (0 based). |
| `gestures` | see below | Hold the button and move the mouse. |
| `macro` | `steps` | A list of steps: `{ step = "keys", keys = [...] }`, `{ step = "builtin", id = "..." }`, `{ step = "click", button = "left" }`, `{ step = "delay", ms = 100 }`, `{ step = "command", command = "..." }`. |

### Gestures

```toml
[devices."b034-5C8F7AF8".bindings.buttons.gesture]
type = "gestures"
mode = "directional"        # or "pan": move the mouse to scroll in any direction
up = { type = "builtin", id = "overview" }
down = { type = "builtin", id = "show-desktop" }
left = { type = "builtin", id = "workspace-right" }
right = { type = "builtin", id = "workspace-left" }
click = { type = "builtin", id = "overview" }   # press and release without moving
threshold_mm = 7.0          # distance before a direction is recognised
trigger = "move"            # or "release"
repeat_vertical = false     # keep firing up/down while moving (volume, zoom)
repeat_horizontal = false
pan_speed = 1.0             # pan mode
pan_invert = false          # pan mode: drag the content instead of the view
```

Gestures work on every control with raw motion reporting — on MX mice that is the gesture
button, the middle, back and forward buttons and the mode shift button.

## Built-in actions

Desktop actions are performed the way each desktop expects; a few are not available
everywhere (`mouse-masterctl actions` lists what works on yours).

| id | Action | Category |
| --- | --- | --- |
| `left-click` | Left click | Mouse |
| `right-click` | Right click | Mouse |
| `middle-click` | Middle click | Mouse |
| `double-click` | Double click | Mouse |
| `scroll-up` | Scroll up | Mouse |
| `scroll-down` | Scroll down | Mouse |
| `scroll-left` | Scroll left | Mouse |
| `scroll-right` | Scroll right | Mouse |
| `back` | Back | Navigation |
| `forward` | Forward | Navigation |
| `page-up` | Page up | Navigation |
| `page-down` | Page down | Navigation |
| `home` | Home | Navigation |
| `end` | End | Navigation |
| `enter` | Enter | Navigation |
| `escape` | Escape | Navigation |
| `app-switcher` | Switch applications | Navigation |
| `copy` | Copy | Editing |
| `cut` | Cut | Editing |
| `paste` | Paste | Editing |
| `undo` | Undo | Editing |
| `redo` | Redo | Editing |
| `select-all` | Select all | Editing |
| `find` | Find | Editing |
| `save` | Save | Editing |
| `delete` | Delete | Editing |
| `new-tab` | New tab | Browser |
| `close-tab` | Close tab | Browser |
| `reopen-tab` | Reopen closed tab | Browser |
| `next-tab` | Next tab | Browser |
| `previous-tab` | Previous tab | Browser |
| `refresh` | Refresh | Browser |
| `fullscreen` | Full screen | Browser |
| `zoom-in` | Zoom in | Zoom |
| `zoom-out` | Zoom out | Zoom |
| `zoom-reset` | Reset zoom | Zoom |
| `play-pause` | Play / pause | Media |
| `next-track` | Next track | Media |
| `previous-track` | Previous track | Media |
| `stop-media` | Stop | Media |
| `volume-up` | Volume up | Media |
| `volume-down` | Volume down | Media |
| `mute` | Mute | Media |
| `mic-mute` | Mute microphone | Media |
| `overview` | Overview | Desktop |
| `show-desktop` | Show desktop | Desktop |
| `present-windows` | Present windows | Desktop |
| `present-all-windows` | Present all windows | Desktop |
| `desktop-grid` | Desktop grid | Desktop |
| `workspace-left` | Previous desktop | Desktop |
| `workspace-right` | Next desktop | Desktop |
| `workspace-up` | Desktop above | Desktop |
| `workspace-down` | Desktop below | Desktop |
| `app-launcher` | Application launcher | Desktop |
| `search` | Search | Desktop |
| `emoji-picker` | Emoji picker | Desktop |
| `clipboard-history` | Clipboard history | Desktop |
| `screenshot` | Screenshot | Desktop |
| `screenshot-region` | Screenshot of a region | Desktop |
| `close-window` | Close window | Window |
| `maximize-window` | Maximize window | Window |
| `minimize-window` | Minimize window | Window |
| `fullscreen-window` | Full screen window | Window |
| `tile-left` | Tile window left | Window |
| `tile-right` | Tile window right | Window |
| `toggle-floating` | Toggle floating | Window |
| `window-next-desktop` | Move window to next desktop | Window |
| `window-previous-desktop` | Move window to previous desktop | Window |
| `brightness-up` | Brightness up | System |
| `brightness-down` | Brightness down | System |
| `lock-screen` | Lock screen | System |
| `screen-off` | Turn off screen | System |
| `night-light` | Toggle night light | System |
| `do-not-disturb` | Toggle do not disturb | System |
| `calculator` | Calculator | Apps |
| `file-manager` | File manager | Apps |
| `mouse-master` | Mouse Master | Apps |
| `dpi-cycle` | Cycle pointer speed | Device |
| `dpi-up` | Pointer speed up | Device |
| `dpi-down` | Pointer speed down | Device |
| `wheel-mode-toggle` | Toggle ratchet / free spin | Device |
| `smartshift-toggle` | Toggle SmartShift | Device |
| `invert-scroll-toggle` | Toggle scroll direction | Device |
| `show-battery` | Show battery level | Device |
| `next-host` | Switch to next computer | Device |
