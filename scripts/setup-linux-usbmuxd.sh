#!/usr/bin/env bash
set -euo pipefail
# Installs and enables usbmuxd on Debian/Ubuntu. Used in the README's
# "Linux: prerequisites" section. macOS users do not need this; the daemon
# ships with macOS as part of "Apple Mobile Device" launchd service.
if ! command -v apt-get >/dev/null; then
  echo "Non-apt distro detected; please install 'usbmuxd' via your package manager." >&2
  exit 0
fi
sudo apt-get update
sudo apt-get install -y usbmuxd libimobiledevice6 libimobiledevice-utils
sudo systemctl enable --now usbmuxd
echo "OK. Plug an iPhone and run 'idevice_id -l' to verify."
