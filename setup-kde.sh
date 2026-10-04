#!/usr/bin/env bash
# Installation script for ASUS Zenbook Duo Linux dual-screen management (KDE).
# Installs dependencies, configures sudoers and udev rules, and installs the
# Rust runtime services.
set -euo pipefail

DUO_SETUP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-${0}}")" && pwd)"
SETUP_SCRIPT_NAME="setup-kde.sh"
DNF_DESKTOP_PACKAGES=(kscreen)
APT_DESKTOP_PACKAGES=(kscreen)
PACMAN_DESKTOP_PACKAGES=(kscreen kdialog qt6-tools plasma-desktop wireplumber libpulse)
MANUAL_DESKTOP_DEPENDENCIES_HINT="kscreen/kscreen-doctor, qdbus6, kdialog, plasma-emojier, wpctl"

# shellcheck source=setup-common.sh
source "${DUO_SETUP_DIR}/setup-common.sh"
run_duo_setup "$@"
