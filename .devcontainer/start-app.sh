#!/usr/bin/env bash
# Runs at every Codespace start: launch the desktop app inside the web desktop (DISPLAY=:1),
# so that opening the forwarded port 6080 shows the application window without any further steps.
cd "$(dirname "$0")/.." || exit 0
export DISPLAY=:1
export PATH="/usr/local/share/nvm/current/bin:/usr/local/cargo/bin:$HOME/.cargo/bin:$PATH"
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
# [t]arget: the bracket keeps this pgrep from matching its own (or a caller's) command line.
if pgrep -f "[t]arget/debug/guarded-upgrade-path" >/dev/null 2>&1; then
  echo "App already running."
  exit 0
fi
# setsid + nohup: survive the end of the Codespaces lifecycle command that launches this script.
setsid nohup bash -c 'exec npm run tauri dev' >/tmp/tauri-dev.log 2>&1 </dev/null &
disown || true
echo "Starting the Guarded Upgrade Path app (first start compiles for a few minutes). Open port 6080, password 'vscode'. Log: /tmp/tauri-dev.log"
