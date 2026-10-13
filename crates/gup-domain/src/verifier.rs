//! Independent verification.
//!
//! The verifier takes the tagged artifact inputs (manifest JSON and payload
//! bytes), recomputes `H`, compares it with an expected value, checks that
//! the manifest describes the payload, verifies guardian signatures over `H`
//! and evaluates the quorum. It never reads the proposing agent's summary or
//! its claimed hash. The same code backs the `gup-verify` CLI, which runs
//! out of process on files the reviewer controls.

use crate::artifact::ArtifactManifest;
use crate::canonical::{canonical_bytes, compute_commitment, sha256_hex, Commitment};
use crate::checks::manifest_binding_check;
use crate::guardians::{verify_approval, Authorization, Decision, GuardianSet};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierStep {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureCheck {
    pub guardian_id: String,
    pub decision: Decision,
    pub commitment: Commitment,
    pub ok: bool,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Accept,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierReport {
    pub verdict: Verdict,
    pub computed_commitment: Option<Commitment>,
    pub expected_commitment: Option<Commitment>,
    pub manifest_sha256: String,
    pub payload_sha256: String,
    pub manifest_canonical_len: usize,
    pub payload_len: usize,
    pub steps: Vec<VerifierStep>,
    pub signatures: Vec<SignatureCheck>,
    pub quorum_have: usize,
    pub quorum_need: usize,
    pub reasons: Vec<String>,
    pub what_this_establishes: String,
    pub what_this_does_not_establish: String,
}

fn step(steps: &mut Vec<VerifierStep>, name: &str, ok: bool, detail: impl Into<String>) {
    steps.push(VerifierStep {
        name: name.into(),
        ok,
        detail: detail.into(),
    });
}

/// Verify artifact inputs. `manifest_json` may be formatted any way; it is
/// parsed and canonicalized. `expected` is the commitment the guardians
/// approved (from the authorization or supplied by the operator).
pub fn verify(
    manifest_json: &[u8],
    payload: &[u8],
    expected: Option<Commitment>,
    authorization: Option<&Authorization>,
    guardians: Option<&GuardianSet>,
) -> VerifierReport {
    let mut steps = Vec::new();
    let mut reasons = Vec::new();
    let mut signatures = Vec::new();
    let manifest_sha256 = sha256_hex(manifest_json);
    let payload_sha256 = sha256_hex(payload);
    let mut computed: Option<Commitment> = None;
    let mut canonical_len = 0usize;
    let mut quorum_have = 0usize;
    let mut quorum_need = 0usize;

    let manifest: Option<ArtifactManifest> =
        match serde_json::from_slice::<ArtifactManifest>(manifest_json) {
            Ok(m) => {
                step(
                    &mut steps,
                    "parse manifest",
                    true,
                    format!(
                        "schema {}, package {} {} -> {}",
                        m.schema, m.package, m.from_version, m.to_version
                    ),
                );
                Some(m)
            }
            Err(e) => {
                step(&mut steps, "parse manifest", false, e.to_string());
                reasons.push(format!("manifest unparseable: {e}"));
                None
            }
        };

    if let Some(m) = &manifest {
        match canonical_bytes(m) {
            Ok(canon) => {
                canonical_len = canon.len();
                let h = compute_commitment(&canon, payload);
                computed = Some(h);
                step(
                    &mut steps,
                    "recompute H",
                    true,
                    format!(
                        "H = {} over {} canonical manifest bytes and {} payload bytes",
                        h.hex(),
                        canon.len(),
                        payload.len()
                    ),
                );
            }
            Err(e) => {
                step(&mut steps, "recompute H", false, e.to_string());
                reasons.push(format!("manifest not canonicalizable: {e}"));
            }
        }
        let binding = manifest_binding_check(m, payload);
        step(
            &mut steps,
            "manifest describes payload",
            binding.passed,
            binding.output.clone(),
        );
        if !binding.passed {
            reasons.push(format!("manifest/payload mismatch: {}", binding.output));
        }
    }

    let expected_eff = expected.or(authorization.map(|a| a.commitment));
    match (computed, expected_eff) {
        (Some(c), Some(e)) => {
            let ok = c == e;
            step(
                &mut steps,
                "compare with expected H",
                ok,
                if ok {
                    format!("computed {} equals expected", c.hex())
                } else {
                    format!("computed {} differs from expected {}", c.hex(), e.hex())
                },
            );
            if !ok {
                reasons.push(
                    "the artifact handed to the verifier is not the artifact that was approved"
                        .into(),
                );
            }
        }
        (Some(_), None) => step(
            &mut steps,
            "compare with expected H",
            true,
            "no expected value supplied; reporting computed H only",
        ),
        _ => {}
    }

    if let Some(auth) = authorization {
        quorum_need = auth.policy.m;
        match guardians {
            None => {
                step(
                    &mut steps,
                    "verify guardian signatures",
                    false,
                    "no guardian set supplied",
                );
                reasons.push("cannot verify signatures without the guardian public keys".into());
            }
            Some(set) => {
                let mut counted: Vec<String> = Vec::new();
                for a in &auth.approvals {
                    let r = verify_approval(set, a);
                    let sig_ok = r.is_ok();
                    let binds = computed.map(|c| c == a.commitment).unwrap_or(false);
                    let same_proposal =
                        a.proposal_id == auth.proposal_id && a.revision == auth.revision;
                    let ok = sig_ok && binds && same_proposal && a.decision == Decision::Approve;
                    let detail = match &r {
                        Err(e) => e.to_string(),
                        Ok(()) if !same_proposal => {
                            "signature valid but for a different proposal or revision".into()
                        }
                        Ok(()) if !binds => format!(
                            "signature valid but over {} rather than the recomputed H",
                            a.commitment.hex()
                        ),
                        Ok(()) if a.decision != Decision::Approve => {
                            format!("signature valid; decision is {}", a.decision.label())
                        }
                        Ok(()) => "valid Ed25519 signature over exactly H".into(),
                    };
                    signatures.push(SignatureCheck {
                        guardian_id: a.guardian_id.clone(),
                        decision: a.decision,
                        commitment: a.commitment,
                        ok,
                        detail,
                    });
                    if ok && !counted.contains(&a.guardian_id) {
                        counted.push(a.guardian_id.clone());
                    }
                }
                quorum_have = counted.len();
                let met = quorum_have >= quorum_need && quorum_need > 0;
                step(
                    &mut steps,
                    "verify guardian signatures",
                    signatures.iter().all(|s| s.ok),
                    format!(
                        "{} of {} approvals verify over the recomputed H",
                        signatures.iter().filter(|s| s.ok).count(),
                        signatures.len()
                    ),
                );
                step(&mut steps, "quorum", met, format!("{quorum_have} valid approvals of exactly H; policy {} requires {quorum_need}", auth.policy.label));
                if !met {
                    reasons.push(format!(
                        "quorum not met for the recomputed H ({quorum_have} of {quorum_need})"
                    ));
                }
            }
        }
    }

    let verdict = if reasons.is_empty() && steps.iter().all(|s| s.ok) {
        Verdict::Accept
    } else {
        Verdict::Reject
    };
    VerifierReport {
        verdict,
        computed_commitment: computed,
        expected_commitment: expected_eff,
        manifest_sha256,
        payload_sha256,
        manifest_canonical_len: canonical_len,
        payload_len: payload.len(),
        steps,
        signatures,
        quorum_have,
        quorum_need,
        reasons,
        what_this_establishes: "That the manifest and payload handed to the verifier hash to H, that the manifest describes this payload, and (when supplied) that the listed guardians signed exactly H.".into(),
        what_this_does_not_establish: "That the change is correct, safe or desirable; that the guardians read the package; that the keys belong to real guardians (local test keys in this demo).".into(),
    }
}
