#!/usr/bin/env bash
# Removes the ccdash-daemon service for the current user.
set -euo pipefail

uname_s="$(uname -s)"
case "$uname_s" in
  Darwin)
    PLIST_DEST="${HOME}/Library/LaunchAgents/com.ccdash.daemon.plist"
    if [ -f "${PLIST_DEST}" ]; then
      launchctl unload "${PLIST_DEST}" 2>/dev/null || true
      rm -f "${PLIST_DEST}"
      echo "ccdash-daemon: launchd service removed"
    fi
    ;;
  Linux)
    systemctl --user disable --now ccdash-daemon.service 2>/dev/null || true
    rm -f "${HOME}/.config/systemd/user/ccdash-daemon.service"
    systemctl --user daemon-reload 2>/dev/null || true
    echo "ccdash-daemon: systemd user service removed"
    ;;
esac
