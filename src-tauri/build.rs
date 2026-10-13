fn main() {
    // The Tauri shell is built for Windows and macOS only. Every other target
    // gets the refusal stub in src/unsupported.rs and needs no Tauri context.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" || target_os == "macos" {
        tauri_build::build()
    } else {
        println!(
            "cargo:warning=Guarded Upgrade Path runs on Windows and macOS only; the binary for `{target_os}` refuses to start."
        );
    }
}
