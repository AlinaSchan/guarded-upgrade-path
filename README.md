# Guarded Upgrade Path — interactive technical review sandbox

A desktop review aid by [Logos](https://logos.co/), built around the *Guarded Upgrade Path* review draft
(October 2026, prepared for Bertrand Juglas). It makes the draft's model
operable so that its boundaries can be examined: an autonomous agent
prepares an upgrade and commits to an artifact hash `H`; designated human
guardians recompute `H` and approve exactly `H`, reject it, or request
changes; a constrained executor applies only the authorized `H`. The four
upgrade cases, the evidence package with its four evidence classifications,
the approval-surface experiment, the recovery model, the incident authority
ladder and the six failure modes from the draft are all executable through a
Rust domain engine, not drawn in the UI.

**Status.** This is a review aid, not production software. It runs offline on
a deterministic local model. No external review of it has been completed.
The hypothesis it explores is not presented as an established or universally
safe governance model.

## Download (Windows and macOS)

Installers are on the releases page:
**https://github.com/AlinaSchan/guarded-upgrade-path/releases/latest**

* **Windows 10/11 (x64):** the `*_x64-setup.exe` installer (installs per user,
  no administrator rights) or the `*_x64_en-US.msi`. The installer is unsigned,
  so SmartScreen may show "Windows protected your PC" once: *More info → Run
  anyway*.
* **macOS:** `*_aarch64.dmg` for Apple silicon (M1 or later), `*_x64.dmg` for
  Intel. Drag the app to Applications. The app is unsigned and not notarized,
  so the first launch is blocked by Gatekeeper: either *System Settings →
  Privacy & Security → Open Anyway* after the first attempt, or clear the
  quarantine flag once:

  ```bash
  xattr -dr com.apple.quarantine "/Applications/Guarded Upgrade Path.app"
  ```

* **Linux and web browsers are not supported.** No Linux installer exists, and
  on any platform other than Windows or macOS the application refuses to start
  with an error (`src-tauri/src/unsupported.rs`). The interface is not a web
  application: opened outside the desktop shell it only shows a notice.

The verifier CLI (`gup-verify`) for Windows and macOS is attached to the same
release.

## What is genuine and what is simulated

* Genuine: recomputation of `H` (SHA-256 over a canonical manifest and the
  raw payload), Ed25519 signature verification over `H`, the
  manifest–payload binding check, structural checks on the parsed payload,
  and the authority boundaries enforced in Rust.
* Simulated: the protocol, the payload (deterministic example data standing
  in for bytecode), the replay and fuzzing, every figure, and guardian
  identity (local test keys that the demo itself holds).

Details: [docs/VERIFICATION.md](docs/VERIFICATION.md) and
[docs/SIMULATION_SCOPE.md](docs/SIMULATION_SCOPE.md).

## Repository layout

```
crates/gup-domain   domain engine (Rust library)
crates/gup-verify   independent verifier CLI (Rust binary)
src-tauri           Tauri 2 desktop shell (Windows and macOS only)
src                 React + TypeScript frontend
docs                traceability, verification, simulation scope, policies, reviewer guide, architecture
.github/workflows   CI and draft-release packaging
```

## Building and running

Prerequisites: a Rust stable toolchain, Node.js 24 (LTS), and the Tauri 2
platform prerequisites (on Windows: Microsoft C++ Build Tools and the WebView2
runtime; on macOS: Xcode command line tools). The desktop application builds
and runs on Windows and macOS only; on any other platform `src-tauri` compiles
to a stub that prints an error and exits.

```bash
npm ci
npm run tauri dev          # development
npm run tauri build        # installers for the current platform
cargo test                 # Rust tests (domain engine + CLI)
npm test                   # frontend unit tests
cargo build --release -p gup-verify   # the verifier CLI
```

### Building without MSVC (as on the machine this was developed on)

The official Windows installers come from the GitHub Actions workflow on a
standard MSVC runner. Locally, without administrator rights, the project also
builds and packages with the `x86_64-pc-windows-gnu` toolchain plus a complete
MinGW-w64 GCC (for example the WinLibs POSIX/MSVCRT package installed per
user via winget) on `PATH`; rustup's self-contained MinGW alone is not enough
for Tauri because its `dlltool` cannot run. A GNU-linked binary loads
`WebView2Loader.dll` dynamically; the bundler places that file next to the
executable in the installers.

## Packaging and releases

* Windows: NSIS installer (per-user) and MSI. macOS: `.app` and `.dmg`
  (Apple silicon and Intel). Configured in `src-tauri/tauri.*.conf.json`.
* Linux: not supported. No Linux installer is built (`tauri.linux.conf.json`
  disables bundling; no Linux job exists), and on any target other than
  Windows or macOS `src-tauri` compiles to a stub that prints an error and
  exits (`src-tauri/src/unsupported.rs`; Tauri itself is not compiled there,
  see the target-specific dependencies in `src-tauri/Cargo.toml`). CI checks
  that refusal on Linux.
* `.github/workflows/release.yml` builds on a `v*` tag and creates a
  **draft** release; the draft is published by hand after the assets have
  been checked. Installers are unsigned and not notarized.
* `.github/workflows/ci.yml` runs formatting, Clippy, Rust tests, the Linux
  refusal check, the frontend type-check, bundle and tests, and a packaging
  smoke build on demand.

## Using the verifier outside the application

From the Execution screen, export a verification bundle, then:

```bash
gup-verify hash          --manifest manifest.json --payload payload.bin
gup-verify verify        --manifest manifest.json --payload payload.bin --expected $(cat expected_commitment.txt)
gup-verify authorization --manifest manifest.json --payload payload.bin --authorization authorization.json --guardians guardians.json
gup-verify preimage      --manifest manifest.json --payload payload.bin --out preimage.bin && sha256sum preimage.bin
```

Exit code 0 means verified, 2 means rejected.

## Local data

The workspace (`workspace.json`) and an append-only audit log
(`audit.jsonl`) live in the application data directory (shown on the *Scope
& simulation* screen). Nothing leaves the machine; exports go only where the
reviewer chooses.

## Review scope

The external reviewer is expected to provide professional judgment based on
public systems and the presented model. Not required: confidential
information from FeverTokens, GoodDollar or any other organization; private
source code; production or wallet access; credentials; deployment; live
governance participation; audit or certification. This program does not
redefine the agreed engagement. See
[docs/REVIEWER_GUIDE.md](docs/REVIEWER_GUIDE.md).

## Documentation

* [docs/PDF_REQUIREMENTS_TRACEABILITY.md](docs/PDF_REQUIREMENTS_TRACEABILITY.md) — each requirement of the draft mapped to files and tests, plus missing information and assumptions
* [docs/VERIFICATION.md](docs/VERIFICATION.md) — canonical format, `H`, signatures, the verifier
* [docs/SIMULATION_SCOPE.md](docs/SIMULATION_SCOPE.md) — exactly what the local model does
* [docs/POLICIES.md](docs/POLICIES.md) — every configurable policy, labelled as illustrative
* [docs/REVIEWER_GUIDE.md](docs/REVIEWER_GUIDE.md) — a 15-minute walk-through
* [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — crates, modules, state machine

## Identity

The interface uses the Logos identity: the lambda mark and the "Logos" wordmark
(geometry from logos.co, `src/components/LogosBrand.tsx`), the logos.co palette
and the open Fira Code and Public Sans typefaces (OFL, self-hosted through
`@fontsource`). The Logos name, mark and wordmark belong to Logos.

## License

MIT. Copyright © 2026 [Logos](https://logos.co/). See [LICENSE](LICENSE).
