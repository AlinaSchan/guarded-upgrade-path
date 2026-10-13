//! Refusal path for every platform the review sandbox is not distributed for.
//!
//! Guarded Upgrade Path runs on Windows and macOS only. On any other target
//! this module is the whole program: `main` calls [`refuse`], which reports
//! the error (on stderr, and in a desktop dialog when a common dialog tool is
//! installed) and exits with a non-zero status. The engine is never loaded
//! and no window is opened.

use std::process::{Command, Stdio};

/// The error shown on an unsupported platform.
pub const MESSAGE: &str =
    "Guarded Upgrade Path runs on Windows and macOS only; it does not run on \
this operating system. Download the Windows or macOS installer from \
https://github.com/AlinaSchan/guarded-upgrade-path/releases/latest";

/// Exit status of the refusal.
pub const EXIT_CODE: i32 = 1;

pub fn refuse() -> ! {
    eprintln!("error: {MESSAGE}");
    show_dialog();
    std::process::exit(EXIT_CODE)
}

/// Best effort: show the message in a desktop dialog through the first dialog
/// tool that can be started. The exit status of the tool does not matter (it
/// also reflects how the dialog was closed); a tool that cannot be started is
/// skipped.
fn show_dialog() {
    const TITLE: &str = "Guarded Upgrade Path";
    let attempts: [(&str, Vec<String>); 3] = [
        (
            "zenity",
            vec![
                "--error".into(),
                format!("--title={TITLE}"),
                format!("--text={MESSAGE}"),
            ],
        ),
        (
            "kdialog",
            vec![
                "--title".into(),
                TITLE.into(),
                "--error".into(),
                MESSAGE.into(),
            ],
        ),
        (
            "xmessage",
            vec!["-center".into(), format!("{TITLE}: {MESSAGE}")],
        ),
    ];
    for (program, args) in attempts {
        let started = Command::new(program)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok();
        if started {
            return;
        }
    }
}
