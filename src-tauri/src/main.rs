//! Guarded Upgrade Path — entry point.
//!
//! The review sandbox is distributed for Windows and macOS only. On those two
//! targets the desktop shell (`desktop.rs`) starts. On every other target the
//! binary is a stub that reports an error and exits (`unsupported.rs`); the
//! engine, Tauri and its platform libraries are not even compiled there
//! (target-specific dependencies in `Cargo.toml`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(any(windows, target_os = "macos"))]
mod desktop;
#[cfg(not(any(windows, target_os = "macos")))]
mod unsupported;

fn main() {
    #[cfg(any(windows, target_os = "macos"))]
    desktop::run();
    #[cfg(not(any(windows, target_os = "macos")))]
    unsupported::refuse();
}
