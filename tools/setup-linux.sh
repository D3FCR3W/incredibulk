#!/usr/bin/env bash
# Native dependencies for the desktop app on Ubuntu and Debian.
set -euo pipefail

if [[ "$(uname -s)" != Linux ]] || ! command -v apt-get >/dev/null; then
  echo "This script supports Linux distributions using apt-get (Ubuntu/Debian)." >&2
  exit 1
fi

as_root=()
if (( EUID != 0 )); then
  if ! command -v sudo >/dev/null; then
    echo "Run this script as root, or install sudo first." >&2
    exit 1
  fi
  as_root=(sudo)
fi

"${as_root[@]}" apt-get update
"${as_root[@]}" apt-get install --yes --no-install-recommends \
  build-essential \
  pkg-config \
  libglib2.0-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  libdbus-1-dev \
  libxdo-dev \
  libxkbcommon-dev

pkg-config --modversion glib-2.0 gtk+-3.0 webkit2gtk-4.1 dbus-1 xkbcommon
echo "Native dependencies are ready. With Rust installed, run: cargo test --workspace"
