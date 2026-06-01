#!/bin/bash
# Installe les bibliothèques système requises pour compiler les projets Bevy
# (wayland, xkbcommon, ALSA, udev). Sans elles, les build scripts de
# wayland-sys / alsa-sys / libudev-sys échouent (pkg-config introuvable).
set -euo pipefail

# Ne s'exécute que dans l'environnement distant (Claude Code on the web).
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
  SUDO="sudo"
fi

export DEBIAN_FRONTEND=noninteractive

$SUDO apt-get update -y
$SUDO apt-get install -y --no-install-recommends \
  pkg-config \
  libwayland-dev \
  libxkbcommon-dev \
  libasound2-dev \
  libudev-dev

echo "Dependances systeme Bevy installees."
