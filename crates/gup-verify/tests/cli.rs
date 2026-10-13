//! Out-of-process verification: the CLI reads only files on disk.

use gup_domain::canonical::{sha256, Commitment};
use gup_domain::engine::{TamperVariant, Workspace};
use gup_domain::guardians::Decision;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_gup-verify")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("gup-verify-test-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(bin())
        .args(args)
        .output()
        .expect("run gup-verify");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn write_bundle(ws: &Workspace, dir: &Path) -> (String, String, String, String) {
    for (name, bytes) in ws.verification_bundle("A").unwrap() {
        fs::write(dir.join(name), bytes).unwrap();
    }
    let s = |n: &str| dir.join(n).to_string_lossy().into_owned();
    (
        s("manifest.json"),
        s("payload.bin"),
        s("authorization.json"),
        s("guardians.json"),
    )
}

#[test]
fn verifies_exported_bundle_and_detects_substitution() {
    let mut ws = Workspace::new();
    ws.agent_run_to_submission("A").unwrap();
    ws.guardian_decide("A", "guardian-1", Decision::Approve, "")
        .unwrap();
    ws.guardian_decide("A", "guardian-2", Decision::Approve, "")
        .unwrap();
    let h = ws
        .scenario("A")
        .unwrap()
        .authorization
        .as_ref()
        .unwrap()
        .commitment;
    let dir = tmp("ok");
    let (m, p, a, g) = write_bundle(&ws, &dir);
    let pre = dir.join("preimage.bin").to_string_lossy().into_owned();

    let (code, out, _) = run(&["hash", "--manifest", &m, "--payload", &p]);
    assert_eq!(code, 0);
    assert_eq!(out.lines().next().unwrap().trim(), h.hex());

    let (code, out, _) = run(&[
        "verify",
        "--manifest",
        &m,
        "--payload",
        &p,
        "--expected",
        &h.hex(),
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("verdict: Accept"));

    let (code, out, _) = run(&[
        "authorization",
        "--manifest",
        &m,
        "--payload",
        &p,
        "--authorization",
        &a,
        "--guardians",
        &g,
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("quorum: 2 of 2"));

    let (code, _, _) = run(&["preimage", "--manifest", &m, "--payload", &p, "--out", &pre]);
    assert_eq!(code, 0);
    let digest = sha256(&fs::read(&pre).unwrap());
    assert_eq!(Commitment(digest), h);

    // Wrong expected value is rejected.
    let (code, out, _) = run(&[
        "verify",
        "--manifest",
        &m,
        "--payload",
        &p,
        "--expected",
        &Commitment([7u8; 32]).hex(),
    ]);
    assert_eq!(code, 2, "{out}");

    // Substitute the payload after approval: the displayed summary is untouched; the files say otherwise.
    ws.tamper("A", TamperVariant::DivertFee).unwrap();
    let dir2 = tmp("tampered");
    let (m2, p2, a2, _) = write_bundle(&ws, &dir2);
    let (code, out, _) = run(&[
        "verify",
        "--manifest",
        &m2,
        "--payload",
        &p2,
        "--expected",
        &h.hex(),
    ]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("verdict: Reject"));
    let (code, out, _) = run(&[
        "authorization",
        "--manifest",
        &m2,
        "--payload",
        &p2,
        "--authorization",
        &a2,
        "--guardians",
        &g,
    ]);
    assert_eq!(code, 2, "{out}");
    let (code, out, _) = run(&["inspect", "--manifest", &m2, "--payload", &p2]);
    assert_eq!(code, 0);
    assert!(out.contains("DOES NOT MATCH"), "{out}");

    // Consistent manifest+payload substitution: internal binding passes, H still differs.
    ws.restore_candidate("A").unwrap();
    ws.tamper("A", TamperVariant::ManifestAndPayload).unwrap();
    let dir3 = tmp("consistent");
    let (m3, p3, _, _) = write_bundle(&ws, &dir3);
    let (code, out, _) = run(&[
        "verify",
        "--manifest",
        &m3,
        "--payload",
        &p3,
        "--expected",
        &h.hex(),
    ]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("[ok] manifest describes payload"));
    assert!(out.contains("[FAIL] compare with expected H"));

    let (code, _, err) = run(&["bogus"]);
    assert_eq!(code, 1);
    assert!(err.contains("unknown command"));
}
