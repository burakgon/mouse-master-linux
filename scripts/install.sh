#!/usr/bin/env bash
# Builds Mouse Master from source and installs it system wide.
#
#   ./scripts/install.sh                 # install to /usr/local
#   ./scripts/install.sh --prefix /usr   # install to /usr
#   ./scripts/install.sh --uninstall     # remove what was installed
#
# The script asks for your password (sudo) to install files, then enables the
# per-user service so your settings are applied at login.
set -euo pipefail

PREFIX=/usr/local
UNINSTALL=0
ENABLE_SERVICE=1
SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD="$SRC/build/release"

usage() {
    sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
    echo "Options: --prefix DIR, --no-service, --uninstall, --help"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix) PREFIX="$2"; shift 2 ;;
        --prefix=*) PREFIX="${1#*=}"; shift ;;
        --no-service) ENABLE_SERVICE=0; shift ;;
        --uninstall) UNINSTALL=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown option: $1" >&2; usage; exit 2 ;;
    esac
done

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
step() { printf '\033[1;34m==>\033[0m \033[1m%s\033[0m\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

SUDO=""
if [[ $EUID -ne 0 ]]; then
    command -v sudo >/dev/null || die "sudo is required to install system files"
    SUDO=sudo
fi

# udev only reads rules from /etc and /usr/lib.
if [[ "$PREFIX" == "/usr" ]]; then
    UDEV_DIR=/usr/lib/udev/rules.d
else
    UDEV_DIR=/etc/udev/rules.d
fi

reload_udev() {
    $SUDO udevadm control --reload || true
    $SUDO udevadm trigger --action=change --subsystem-match=hidraw || true
    $SUDO udevadm trigger --action=change --subsystem-match=misc --sysname-match=uinput || true
}

if [[ $UNINSTALL -eq 1 ]]; then
    step "Stopping the service"
    systemctl --user disable --now mouse-master.service 2>/dev/null || true
    [[ -f "$BUILD/install_manifest.txt" ]] || die "no install manifest in $BUILD; nothing to remove"
    step "Removing installed files"
    while IFS= read -r file; do
        [[ -n "$file" ]] && $SUDO rm -fv "$file"
    done < "$BUILD/install_manifest.txt"
    systemctl --user daemon-reload || true
    reload_udev
    bold "Mouse Master has been removed. Your settings are kept in ~/.config/mouse-master."
    exit 0
fi

step "Checking build requirements"
missing=()
for tool in cargo cmake c++; do
    command -v "$tool" >/dev/null || missing+=("$tool")
done
if ! command -v qmake6 >/dev/null && ! command -v qtpaths6 >/dev/null && [[ ! -d /usr/lib/cmake/Qt6 && ! -d /usr/lib/x86_64-linux-gnu/cmake/Qt6 && ! -d /usr/lib64/cmake/Qt6 ]]; then
    missing+=("Qt 6 development files")
fi
if [[ ${#missing[@]} -gt 0 ]]; then
    echo "Missing: ${missing[*]}"
    echo
    echo "Arch:          sudo pacman -S --needed rust cmake qt6-base qt6-declarative qt6-svg qt6-wayland"
    echo "Fedora:        sudo dnf install cargo cmake gcc-c++ qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtsvg-devel systemd-devel"
    echo "Debian/Ubuntu: sudo apt install cargo cmake g++ qt6-base-dev qt6-declarative-dev libqt6svg6-dev libudev-dev"
    exit 1
fi

step "Building (this takes a minute the first time)"
cmake -S "$SRC" -B "$BUILD" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$PREFIX" -DUDEV_RULES_DIR="$UDEV_DIR" >/dev/null
cmake --build "$BUILD" -j"$(nproc)"

step "Installing to $PREFIX"
$SUDO cmake --install "$BUILD"

step "Granting device access (udev)"
reload_udev

if [[ $ENABLE_SERVICE -eq 1 ]]; then
    step "Starting the service"
    systemctl --user daemon-reload
    systemctl --user enable --now mouse-master.service
    if [[ "${XDG_CURRENT_DESKTOP:-}" == *Hyprland* ]] && ! systemctl --user is-active --quiet graphical-session.target; then
        echo "Hyprland without a systemd session: the service still starts at login (default.target)."
    fi
fi

echo
bold "Mouse Master is installed."
echo "Open it from your application menu, or run: mouse-master"
echo "Command line: mouse-masterctl --help"
