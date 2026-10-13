#!/usr/bin/env bash
# Runs at every Codespace start: launch the desktop app inside the web desktop (DISPLAY=:1),
# so that opening the forwarded port 6080 shows the application window without any further steps.
cd "$(dirname "$0")/.." || exit 0
export DISPLAY=:1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
if pgrep -f "guarded-upgrade-path" >/dev/null 2>&1; then
  echo "App already running."
  exit 0
fi
nohup npm run tauri dev >/tmp/tauri-dev.log 2>&1 &
echo "Starting the Guarded Upgrade Path app (first start compiles for a few minutes). Open port 6080, password 'vscode'. Log: /tmp/tauri-dev.log"
