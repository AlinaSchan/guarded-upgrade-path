#!/usr/bin/env bash
# Codespaces / dev container setup: Tauri 2 Linux prerequisites, toolchain components,
# frontend dependencies, and a first test run of the domain engine.
# Linux installers are never produced here (tauri.linux.conf.json disables bundling);
# the desktop app is run with `npm run tauri dev` inside the noVNC desktop on port 6080.
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  libgtk-3-dev pkg-config patchelf xdg-utils
sudo rm -rf /var/lib/apt/lists/*

# The base image may ship an older stable; Tauri 2.12 needs rustc 1.90 or newer.
rustup update stable || sudo -E env "PATH=$PATH" rustup update stable
rustup default stable
rustup component add clippy rustfmt
rustc --version

npm ci

echo "== Rust tests (domain engine + verifier CLI) =="
cargo test -p gup-domain -p gup-verify

echo "== Frontend type-check, bundle and unit tests =="
npm run build
npm test

echo "== Pre-building the desktop app in dev profile so 'npm run tauri dev' starts quickly =="
cargo build -p guarded-upgrade-path || echo "App prebuild failed; 'npm run tauri dev' will rebuild and show the error."

cat <<'EOF'

Setup complete.
  Run the desktop app:   npm run tauri dev
  See the window:        open the forwarded port 6080 (Desktop / noVNC, password: vscode)
  Verifier CLI:          cargo run -p gup-verify -- help
  Tests:                 cargo test && npm test
EOF
