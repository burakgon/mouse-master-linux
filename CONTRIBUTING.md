# Contributing to Mouse Master

Thank you for helping! Mouse Master aims to be the best way to use a Logitech mouse on Linux,
and that takes many devices, desktops and hands.

## Ways to help

- **Report your device.** Run `mouse-masterctl info --json` and open a
  [device report](https://github.com/burakgon/mouse-master-linux/issues/new?template=device-report.yml).
  Tell us what works and what does not. This is the single most useful contribution.
- **Draw your mouse.** Artwork lives in `app/assets/devices` (SVG, 400×560, top view, no
  filters) and control positions in `app/qml/Layouts.js`. Look at `mx-master.svg` for the style.
- **Add a desktop.** Desktop actions are in `crates/daemon/src/actions/exec.rs` and focus
  tracking in `crates/daemon/src/window.rs`.
- **Translate** (coming soon), improve the docs, or pick an issue labelled *good first issue*.

## Building

```sh
cargo test --workspace                       # all Rust tests, no hardware needed
cargo run -p mouse-masterd -- --demo -v      # the service on simulated devices
cmake -B build -DBUILD_SERVICE=OFF -DCMAKE_BUILD_TYPE=Debug
cmake --build build && ./build/app/mouse-master
```

Stop the installed service first (`systemctl --user stop mouse-master`) when running a
development build; only one instance can own the D-Bus name.

Useful while working on the app:

- `QT_FORCE_STDERR_LOGGING=1` shows QML warnings in the terminal.
- `MM_SCREENSHOT=/tmp/out.png MM_PAGE=buttons MM_CONTROL=gesture ./build/app/mouse-master`
  renders a page to a file and quits.

## Guidelines

- **Never break a mouse.** The service must leave every control it does not need untouched
  and restore what it changed. New features that write to a device need a test in
  `crates/daemon/src/tests.rs` against the simulator (`crates/hidpp/src/sim.rs`).
- **Match the surrounding code.** `cargo fmt` (see `rustfmt.toml`), `cargo clippy`, and for
  QML the existing component style: colours and sizes come from `Theme.qml`.
- **Keep commits focused** and write messages that explain *why*.
- **Respect trademarks**: describe compatibility ("works with MX Master 3S") but do not use
  Logitech logos or product photos.

## Protocol notes

If you add a HID++ feature, document the byte layout in the module, cite where it came from
(Logitech's published documents, Solaar, logiops, or your own captures), and add parsing
tests with real replies from a device where possible.

By contributing you agree that your contributions are licensed under GPL-3.0-or-later.
