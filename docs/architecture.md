# Architecture

Mouse Master is two programs that talk over the session D-Bus:

- **`mouse-masterd`** (Rust) runs in the background as a systemd user service. It owns the
  devices, applies settings, turns button presses and mouse motion into actions, and follows
  the focused application.
- **`mouse-master`** (C++ and QML, Qt 6) is the app. It only displays state and sends
  commands; closing it changes nothing. `mouse-masterctl` is a command line client for the
  same API.

```
                 ┌──────────────────────────── mouse-masterd ────────────────────────────┐
 hidraw (udev) ─▶│ node task ─▶ device actor ─▶ gestures / thumb wheel ─▶ executor ─────┼─▶ uinput
  Bluetooth,     │   (per        (per device)     engines                  │            │
  receivers      │    hidraw)        ▲  │                                  └─ KWin, ────┼─▶ desktop
                 │                    │  ▼                                    Hyprland,  │
                 │  window tracker ───┘ registry ◀──── D-Bus API ◀─────────── Sway      │
                 └──────────────────────────────────────────┼──────────────────────────────┘
                                                             ▼
                                              mouse-master (app), mouse-masterctl
```

## crates/hidpp

An asynchronous implementation of Logitech's HID++ protocol on top of Linux hidraw.

- `hidraw` enumerates Logitech nodes with udev and reads their report descriptors to learn
  which HID++ report types they accept (Bluetooth devices only take long reports).
- `channel::Channel` owns one hidraw node. A reader task matches replies to the single
  request in flight — by device index, feature index, function and software id, including
  HID++ 1.0 and 2.0 error replies — and forwards everything else (events, receiver
  notifications) to whoever opened the channel. Requests use software id `0x0E`; replies to
  other software (the kernel, Solaar, ...) are ignored.
- `device::Device` pings a device and reads its feature table; `features::*` implement the
  individual HID++ 2.0 features as small async functions.
- `receiver` handles Unifying, Bolt, Lightspeed and Nano receivers through HID++ 1.0
  registers and connection notifications.
- `sim` simulates devices and receivers over a `SOCK_SEQPACKET` socket pair. Everything above
  runs unmodified on it, which is what the end-to-end tests and the demo mode use.

## crates/daemon

### Devices

`devices::node` watches udev on a dedicated thread and runs one task per hidraw node. For a
receiver it enables connection notifications, asks for the list of paired devices and starts
a device actor per slot; for a directly connected device it starts one actor.

Each **device actor** (`devices::actor`) is a task that owns everything about one device:

1. **Initialisation** reads identity, features, controls and current state, publishes a
   snapshot to the registry, applies the stored settings and the bindings of the current
   application profile. A device that is asleep is initialised when it wakes up.
2. **Events**: diverted button presses, raw motion, thumb wheel rotation, battery and
   ratchet changes, and the wireless status event that says the device lost its settings.
3. **Commands** from D-Bus: settings, bindings, profiles, host switching, haptics.
4. **Focus changes**: the matching profile is merged over the default bindings and only the
   differences are written to the device. A change is deferred while a diverted control is
   held, so its release is never lost.

Only controls with a non-default binding are diverted; everything else keeps its native
behaviour, even while the service runs. On exit the actor un-diverts everything it touched.
If the service crashes, systemd restarts it and the new instance reads the device's state and
corrects it.

### Actions

`actions::Action` is the model shared with the app (serialized with a `type` tag).
`actions::exec::Executor` performs actions:

- keys, mouse buttons and scrolling through two uinput devices (`input`), a keyboard and a
  pointer with high resolution wheel axes, so libinput classifies them correctly;
- desktop actions through the desktop's own interface: KWin global shortcuts over D-Bus on
  Plasma, dispatchers over the IPC socket on Hyprland, `swaymsg` on Sway, standard shortcuts
  on GNOME;
- commands and applications in the user's session environment (read from the systemd user
  manager, or launched through Hyprland so window rules apply);
- device operations (DPI, SmartShift, host switching) are handed back to the actor.

`gestures` and `thumbwheel` are pure state machines with their own unit tests.

### Focus tracking

`window` produces the focused application:

- **KDE Plasma**: a small KWin script is installed through KWin's scripting D-Bus API. It
  calls back `WindowTracker1.Activated` on every activation.
- **Hyprland**: `activewindow` events from the event socket.
- **Sway**: `window` events over i3 IPC.

The tracker re-detects the desktop when its backend goes away.

## D-Bus API

Bus name `io.github.burakgon.MouseMaster.Daemon`, object `/io/github/burakgon/MouseMaster/Daemon`,
interface `io.github.burakgon.MouseMaster.Daemon1`. Structured values are JSON strings.

| Method | Description |
| --- | --- |
| `ListDevices() → s` | Snapshots of all devices seen in this session. |
| `GetDevice(s id) → s` | One snapshot. |
| `Command(s id, s command) → s` | A device command, e.g. `{"command":"set-settings","settings":{"dpi":1600}}`. See `devices::Command`. |
| `GetCatalog() → s` | Built-in actions and whether each is available on this desktop. |
| `GetConfig() → s`, `SetConfig(s)` | Export and import the whole configuration (JSON or TOML). |
| `GetGeneral() → s`, `SetGeneral(s patch)` | General settings. |
| `ForgetDevice(s id)` | Removes a disconnected device and its settings. |
| `SetMonitoring(b)` | Live press and motion signals; stops when the caller disconnects. |
| `RunAction(s action)` | Performs an action that does not need a device. |

| Signal | Description |
| --- | --- |
| `DevicesChanged()` | A device appeared or was forgotten. |
| `DeviceChanged(s id, s snapshot)` | A device's state changed. |
| `ButtonEvent(s id, s control, b pressed)` | A control was pressed or released. |
| `GestureEvent(s id, s control, s gesture)` | A gesture was recognised (`up`, `down`, `left`, `right`, `click`). |
| `MotionEvent(s id, i dx, i dy)` | Raw motion while a gesture button is held (monitoring only). |
| `FocusChanged(s app)` | The focused application changed. |

Properties: `Version`, `Desktop`, `InputAvailable`, `ActiveApp`.

## The app

`app/src` contains the C++ glue: `DaemonClient` mirrors the service state for QML,
`SystemIntegration` manages the user service and device permissions, `AppDirectory` reads
desktop entries for the profile picker, and two image providers serve tinted Lucide icons and
theme icons. Everything else is QML in `app/qml`:

- `components/` — the design system (cards, buttons, toggles, sliders, segmented controls,
  chips, dialogs, the shortcut recorder);
- `stage/` — the device artwork with hotspots and callouts;
- `editors/` — the action picker, gesture editor and thumb wheel editor;
- `pages/` — one file per page.

Device artwork lives in `app/assets/devices` as SVG; `Layouts.js` places each control on it.
Adding a new device family means drawing it and listing its control positions there.

## Testing

`cargo test --workspace` runs the protocol unit tests, the gesture and thumb wheel engines and
the end-to-end suite in `crates/daemon/src/tests.rs`, which drives real device actors with
simulated devices and checks both the resulting device state and the input that would have
been sent to uinput.

For the app, `MM_SCREENSHOT=out.png` renders the window to a file and quits (`MM_PAGE`,
`MM_CONTROL`, `MM_DEVICE` and `MM_THEME` choose what is shown). With `mouse-masterd --demo`
this produces the screenshots in the README.
