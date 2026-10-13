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
src-tauri           Tauri 2 desktop shell
src                 React + TypeScript frontend
docs                traceability, verification, simulation scope, policies, reviewer guide, architecture
.github/workflows   CI and draft-release packaging
```

## Building and running

Prerequisites: a Rust stable toolchain, Node.js 24 (LTS), and the Tauri 2
platform prerequisites (on Windows: Microsoft C++ Build Tools and the WebView2
runtime; on macOS: Xcode command line tools).

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

## GitHub Codespaces

The repository ships a dev container (`.devcontainer/`) with the Rust
toolchain, Node 24, the Tauri 2 Linux prerequisites and a lightweight web
desktop. Open the repository in a Codespace (4 cores recommended); setup
installs dependencies, runs the Rust and frontend tests and pre-builds the
app. Then:

```bash
npm run tauri dev
```

or simply wait: at every start the Codespace launches the app by itself and
opens the forwarded port **6080** (Desktop / noVNC, password `vscode`), where
the application window appears. The first start compiles for a few minutes;
`/tmp/tauri-dev.log` shows progress. The CLI runs with
`cargo run -p gup-verify -- help`. No Linux installer is produced; the
Codespace is for reviewing, running and testing the code.

## Packaging and releases

* Windows: NSIS installer (per-user) and MSI. macOS: `.app` and `.dmg`
  (Apple silicon and Intel). Configured in `src-tauri/tauri.*.conf.json`.
* Linux installers are not built or published (`tauri.linux.conf.json`
  disables bundling; no Linux job exists).
* `.github/workflows/release.yml` builds on a `v*` tag and creates a
  **draft** release; publishing it is a manual step and remains subject to
  explicit authorization. Installers are unsigned.
* `.github/workflows/ci.yml` runs formatting, Clippy, Rust tests, the
  frontend type-check, bundle and tests, and a packaging smoke build.

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

## License

MIT. Copyright © 2026 [Logos](https://logos.co/). See [LICENSE](LICENSE).
