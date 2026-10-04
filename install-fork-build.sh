#!/usr/bin/env bash
# Install an already-built fork on an existing Duo installation, preserving settings.
set -euo pipefail
[[ $EUID != 0 ]] || { echo 'Run as your logged-in desktop user.' >&2; exit 1; }
TASK_REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
TASK_BUILD="$TASK_REPO/ui-tauri-react/src-tauri/target/release"
[[ -f "$TASK_BUILD/fork-build.sha256" ]] || { echo 'Build is not validated yet.' >&2; exit 1; }
(cd "$TASK_BUILD" && sha256sum -c fork-build.sha256)
for cmd in qdbus6 kdialog wpctl pactl plasma-emojier; do
 command -v "$cmd" >/dev/null || { echo "Missing dependency: $cmd. Install KDE desktop dependencies first." >&2; exit 1; }
done
TASK_BACKUP="/usr/local/libexec/zenbook-duo-before-0.4.0-$(date -u +%Y%m%dT%H%M%SZ)"
TASK_RULE=$(mktemp)
trap 'rm -f -- "$TASK_RULE"' EXIT
printf '%s ALL=(root) NOPASSWD: /usr/bin/tee /sys/class/backlight/asus_screenpad/brightness, /usr/bin/tee /sys/class/backlight/card*-eDP-2-backlight/brightness\n' "$(id -un)" > "$TASK_RULE"
sudo -v
sudo visudo -cf "$TASK_RULE"
sudo mkdir -p "$TASK_BACKUP"
sudo cp -a /usr/local/libexec/zenbook-duo "$TASK_BACKUP/runtime"
sudo cp -a /usr/local/bin/zenbook-duo-control "$TASK_BACKUP/zenbook-duo-control"
# Stop the old GUI gracefully; long Linux process names cannot be matched with pgrep -x.
python - <<'PY'
import os,pathlib,signal
for p in pathlib.Path('/proc').iterdir():
 if not p.name.isdigit(): continue
 try:
  if p.stat().st_uid != os.getuid(): continue
  argv=(p/'cmdline').read_bytes().split(b'\0')
  if argv and pathlib.Path(os.fsdecode(argv[0])).name=='zenbook-duo-control':
   os.kill(int(p.name),signal.SIGTERM)
 except (OSError,ValueError): pass
PY
systemctl --user stop zenbook-duo-session-preview.service 2>/dev/null || true
systemctl --user stop zenbook-duo-session-agent.service
rm -f -- "$HOME/.config/systemd/user/zenbook-duo-session-agent.service.d/sprint-preview.conf"
systemctl --user daemon-reload
sudo systemctl stop zenbook-duo-rust-daemon.service
TASK_STOPPED=true
recover_services() {
 rm -f -- "$TASK_RULE"
 if [[ ${TASK_STOPPED:-false} == true ]]; then
  sudo systemctl start zenbook-duo-rust-daemon.service || true
  systemctl --user start zenbook-duo-session-agent.service || true
 fi
}
trap recover_services EXIT
# Stage files alongside their targets, then rename so no running file is partially overwritten.
for bin in zenbook-duo-daemon zenbook-duo-lifecycle zenbook-duo-session-agent zenbook-duo-usb-remap-helper; do
 sudo install -m755 "$TASK_BUILD/$bin" "/usr/local/libexec/zenbook-duo/$bin.new"
 sudo mv "/usr/local/libexec/zenbook-duo/$bin.new" "/usr/local/libexec/zenbook-duo/$bin"
done
sudo install -m755 "$TASK_BUILD/zenbook-duo-control" /usr/local/bin/zenbook-duo-control.new
sudo mv /usr/local/bin/zenbook-duo-control.new /usr/local/bin/zenbook-duo-control
sudo install -o root -g root -m440 "$TASK_RULE" /etc/sudoers.d/zenbook-duo-brightness
sudo visudo -c
sudo systemctl start zenbook-duo-rust-daemon.service
systemctl --user start zenbook-duo-session-agent.service
TASK_STOPPED=false
systemctl --user is-active zenbook-duo-session-agent.service
sudo systemctl is-active zenbook-duo-rust-daemon.service
systemd-run --user --collect --unit="zenbook-duo-ui-$(date +%s)" /usr/local/bin/zenbook-duo-control
printf '\nDuo v0.4.0 installed. Open Desktop & keyboard for wallpapers, typing guard and key actions.\nPrevious binaries: %s\n' "$TASK_BACKUP"
