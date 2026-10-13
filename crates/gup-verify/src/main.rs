//! gup-verify — independent verifier for Guarded Upgrade Path artifacts.
//!
//! Reads the tagged artifact inputs from files the operator controls,
//! recomputes the commitment H, compares it with the expected value, checks
//! that the manifest describes the payload, verifies guardian signatures over
//! H and evaluates the quorum. It never reads a displayed summary.
//!
//! Exit codes: 0 verified, 2 mismatch or rejected, 1 usage or I/O error.

use gup_domain::artifact::ArtifactManifest;
use gup_domain::canonical::{canonical_bytes, commitment_preimage, Commitment};
use gup_domain::guardians::{Authorization, GuardianSet};
use gup_domain::verifier::{verify, Verdict};
use std::collections::BTreeMap;
use std::fs;
use std::process::ExitCode;

const USAGE: &str = "gup-verify — independent verifier for Guarded Upgrade Path artifacts

USAGE:
  gup-verify hash          --manifest <file> --payload <file>
  gup-verify verify        --manifest <file> --payload <file> --expected <0xH>
  gup-verify authorization --manifest <file> --payload <file> --authorization <file> --guardians <file> [--expected <0xH>]
  gup-verify preimage      --manifest <file> --payload <file> --out <file>
  gup-verify inspect       --manifest <file> [--payload <file>]

Options:
  --json     print the full report as JSON (verify, authorization)

The verifier reads only the files given. A displayed summary or a claimed
hash is never an input. Exit code 0 = verified, 2 = rejected, 1 = error.";

fn parse_args(args: &[String]) -> Result<(String, BTreeMap<String, String>, bool), String> {
    let cmd = args.first().cloned().ok_or_else(|| USAGE.to_string())?;
    let mut opts = BTreeMap::new();
    let mut json = false;
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if a == "--json" {
            json = true;
            i += 1;
            continue;
        }
        if let Some(name) = a.strip_prefix("--") {
            let v = args
                .get(i + 1)
                .ok_or_else(|| format!("missing value for --{name}"))?;
            opts.insert(name.to_string(), v.clone());
            i += 2;
        } else {
            return Err(format!("unexpected argument {a}\n\n{USAGE}"));
        }
    }
    Ok((cmd, opts, json))
}

fn need<'a>(opts: &'a BTreeMap<String, String>, k: &str) -> Result<&'a String, String> {
    opts.get(k)
        .ok_or_else(|| format!("--{k} is required\n\n{USAGE}"))
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
}

fn run(args: Vec<String>) -> Result<i32, String> {
    let (cmd, opts, json) = parse_args(&args)?;
    match cmd.as_str() {
        "hash" => {
            let manifest = read(need(&opts, "manifest")?)?;
            let payload = read(need(&opts, "payload")?)?;
            let rep = verify(&manifest, &payload, None, None, None);
            match rep.computed_commitment {
                Some(h) => {
                    println!("{}", h.hex());
                    println!(
                        "manifest sha256 {} ({} canonical bytes); payload sha256 {} ({} bytes)",
                        rep.manifest_sha256,
                        rep.manifest_canonical_len,
                        rep.payload_sha256,
                        rep.payload_len
                    );
                    Ok(if rep.verdict == Verdict::Accept { 0 } else { 2 })
                }
                None => Err(rep.reasons.join("; ")),
            }
        }
        "verify" => {
            let manifest = read(need(&opts, "manifest")?)?;
            let payload = read(need(&opts, "payload")?)?;
            let expected = Commitment::parse(need(&opts, "expected")?)?;
            let rep = verify(&manifest, &payload, Some(expected), None, None);
            print_report(&rep, json);
            Ok(if rep.verdict == Verdict::Accept { 0 } else { 2 })
        }
        "authorization" => {
            let manifest = read(need(&opts, "manifest")?)?;
            let payload = read(need(&opts, "payload")?)?;
            let auth: Authorization = serde_json::from_slice(&read(need(&opts, "authorization")?)?)
                .map_err(|e| format!("authorization.json: {e}"))?;
            let guardians: GuardianSet = serde_json::from_slice(&read(need(&opts, "guardians")?)?)
                .map_err(|e| format!("guardians.json: {e}"))?;
            let expected = match opts.get("expected") {
                Some(e) => Some(Commitment::parse(e)?),
                None => None,
            };
            let rep = verify(
                &manifest,
                &payload,
                expected,
                Some(&auth),
                Some(&guardians.public()),
            );
            print_report(&rep, json);
            Ok(if rep.verdict == Verdict::Accept { 0 } else { 2 })
        }
        "preimage" => {
            let manifest_bytes = read(need(&opts, "manifest")?)?;
            let payload = read(need(&opts, "payload")?)?;
            let out = need(&opts, "out")?;
            let m: ArtifactManifest =
                serde_json::from_slice(&manifest_bytes).map_err(|e| format!("manifest: {e}"))?;
            let canon = canonical_bytes(&m).map_err(|e| e.to_string())?;
            let pre = commitment_preimage(&canon, &payload);
            fs::write(out, &pre).map_err(|e| format!("cannot write {out}: {e}"))?;
            let h = gup_domain::canonical::compute_commitment(&canon, &payload);
            println!(
                "wrote {} preimage bytes to {out}; sha256 of that file is H = {}",
                pre.len(),
                h.hex()
            );
            Ok(0)
        }
        "inspect" => {
            let manifest_bytes = read(need(&opts, "manifest")?)?;
            let m: ArtifactManifest =
                serde_json::from_slice(&manifest_bytes).map_err(|e| format!("manifest: {e}"))?;
            println!("schema       {}", m.schema);
            println!(
                "package      {} {} -> {}",
                m.package, m.from_version, m.to_version
            );
            println!("change kind  {}", m.change_kind.label());
            println!("summary      {}", m.summary);
            println!("functions    {}", m.functions.len());
            for f in &m.functions {
                println!(
                    "  {} {} impl={}{}",
                    f.selector,
                    f.name,
                    f.impl_id,
                    if f.privileged { " [privileged]" } else { "" }
                );
            }
            println!(
                "storage      {} format {} ({} slots)",
                m.storage.namespace,
                m.storage.format_version,
                m.storage.slots.len()
            );
            for s in &m.storage.slots {
                println!("  slot {} {} : {}", s.index, s.name, s.ty);
            }
            println!("parameters   {:?}", m.parameters);
            println!(
                "roles        admin {:?} guardians {:?}",
                m.roles.upgrade_admin, m.roles.guardians
            );
            if let Some(mig) = &m.migration {
                println!(
                    "migration    {} format {} -> {}: {}",
                    mig.id, mig.from_format, mig.to_format, mig.description
                );
            }
            println!("payload sha  {}", m.payload_sha256);
            if let Some(p) = opts.get("payload") {
                let payload = read(p)?;
                let rep = verify(&manifest_bytes, &payload, None, None, None);
                println!(
                    "payload file sha256 {} ({}); manifest describes payload: {}",
                    rep.payload_sha256,
                    if rep.payload_sha256 == m.payload_sha256 {
                        "matches manifest"
                    } else {
                        "DOES NOT MATCH manifest"
                    },
                    rep.steps
                        .iter()
                        .find(|s| s.name == "manifest describes payload")
                        .map(|s| s.ok)
                        .unwrap_or(false)
                );
                if let Some(h) = rep.computed_commitment {
                    println!("H            {}", h.hex());
                }
            }
            Ok(0)
        }
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(0)
        }
        other => Err(format!("unknown command {other}\n\n{USAGE}")),
    }
}

fn print_report(rep: &gup_domain::verifier::VerifierReport, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(rep).unwrap_or_default());
        return;
    }
    for s in &rep.steps {
        println!(
            "[{}] {}: {}",
            if s.ok { "ok" } else { "FAIL" },
            s.name,
            s.detail
        );
    }
    for s in &rep.signatures {
        println!(
            "[{}] signature {} ({}): {}",
            if s.ok { "ok" } else { "FAIL" },
            s.guardian_id,
            s.decision.label(),
            s.detail
        );
    }
    if rep.quorum_need > 0 {
        println!(
            "quorum: {} of {} valid approvals over the recomputed H",
            rep.quorum_have, rep.quorum_need
        );
    }
    if let Some(h) = rep.computed_commitment {
        println!("computed H {}", h.hex());
    }
    if let Some(e) = rep.expected_commitment {
        println!("expected H {}", e.hex());
    }
    println!(
        "verdict: {:?}{}",
        rep.verdict,
        if rep.reasons.is_empty() {
            String::new()
        } else {
            format!(" — {}", rep.reasons.join("; "))
        }
    );
    println!("establishes: {}", rep.what_this_establishes);
    println!("does not establish: {}", rep.what_this_does_not_establish);
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(args) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}
