#!/usr/bin/env bash
# Installs the ccdash-daemon service for the current user. Called by the brew
# formula's `post_install` step. Idempotent.
set -euo pipefail

BREW_PREFIX="${1:-$(brew --prefix)}"
USER_LOG_DIR="${HOME}/Library/Logs"

uname_s="$(uname -s)"
case "$uname_s" in
  Darwin)
    PLIST_SRC="${BREW_PREFIX}/share/ccdash/com.ccdash.daemon.plist"
    PLIST_DEST="${HOME}/Library/LaunchAgents/com.ccdash.daemon.plist"
    mkdir -p "${HOME}/Library/LaunchAgents" "${USER_LOG_DIR}/ccdash"
    sed -e "s|BREW_PREFIX|${BREW_PREFIX}|g" \
        -e "s|USER_LOG_DIR|${USER_LOG_DIR}|g" \
        "${PLIST_SRC}" > "${PLIST_DEST}"
    launchctl unload "${PLIST_DEST}" 2>/dev/null || true
    launchctl load "${PLIST_DEST}"
    echo "ccdash-daemon: launchd service installed and loaded"
    ;;
  Linux)
    UNIT_SRC="${BREW_PREFIX}/share/ccdash/ccdash-daemon.service"
    UNIT_DEST="${HOME}/.config/systemd/user/ccdash-daemon.service"
    mkdir -p "${HOME}/.config/systemd/user"
    sed -e "s|BREW_PREFIX|${BREW_PREFIX}|g" "${UNIT_SRC}" > "${UNIT_DEST}"
    systemctl --user daemon-reload
    systemctl --user enable --now ccdash-daemon.service
    echo "ccdash-daemon: systemd user service installed and started"
    ;;
  *)
    echo "ccdash-daemon: unsupported OS '$uname_s'. Start it manually with:"
    echo "  ${BREW_PREFIX}/bin/ccdash-daemon"
    ;;
esac
