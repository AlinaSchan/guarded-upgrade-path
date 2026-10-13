//! Guardians, quorum policies and signed approvals.
//!
//! Signatures are real Ed25519 signatures over a domain-separated message
//! that binds the proposal id, the decision and the commitment `H`. The keys
//! are **local test keys** derived from fixed seeds; the demo holds all of
//! them, so guardian *identity* is simulated while signature *verification*
//! is genuine. See `docs/VERIFICATION.md`.

use crate::canonical::{sha256, Commitment};
use crate::surface::SurfaceMode;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

pub const APPROVAL_DOMAIN: &str = "GUP-APPROVAL/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guardian {
    pub id: String,
    pub label: String,
    pub public_key_hex: String,
    /// Present only for local test keys. Never for a real guardian.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_secret_hex: Option<String>,
}

impl Guardian {
    pub fn verifying_key(&self) -> Result<VerifyingKey, String> {
        let bytes = hex::decode(&self.public_key_hex).map_err(|e| e.to_string())?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "public key must be 32 bytes".to_string())?;
        VerifyingKey::from_bytes(&arr).map_err(|e| e.to_string())
    }

    fn signing_key(&self) -> Result<SigningKey, String> {
        let secret = self
            .test_secret_hex
            .as_ref()
            .ok_or("no local test secret for this guardian")?;
        let bytes = hex::decode(secret).map_err(|e| e.to_string())?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "secret must be 32 bytes".to_string())?;
        Ok(SigningKey::from_bytes(&arr))
    }

    /// Public view: strips the test secret.
    pub fn public(&self) -> Guardian {
        Guardian {
            test_secret_hex: None,
            ..self.clone()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardianSet {
    pub guardians: Vec<Guardian>,
    pub note: String,
}

impl GuardianSet {
    /// Deterministic local test guardians. Seeds are public by construction.
    pub fn local_test(n: usize) -> Self {
        let guardians = (1..=n)
            .map(|i| {
                let seed = sha256(format!("gup-local-test-key/{i}").as_bytes());
                let sk = SigningKey::from_bytes(&seed);
                Guardian {
                    id: format!("guardian-{i}"),
                    label: format!("Guardian {i} (local test key)"),
                    public_key_hex: hex::encode(sk.verifying_key().to_bytes()),
                    test_secret_hex: Some(hex::encode(seed)),
                }
            })
            .collect();
        GuardianSet {
            guardians,
            note: "Local test keys derived from public seeds. Signature verification is real; guardian identity is simulated because this demo holds every key.".into(),
        }
    }

    pub fn get(&self, id: &str) -> Option<&Guardian> {
        self.guardians.iter().find(|g| g.id == id)
    }

    pub fn public(&self) -> GuardianSet {
        GuardianSet {
            guardians: self.guardians.iter().map(Guardian::public).collect(),
            note: self.note.clone(),
        }
    }

    pub fn len(&self) -> usize {
        self.guardians.len()
    }

    pub fn is_empty(&self) -> bool {
        self.guardians.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumPolicy {
    pub id: String,
    pub label: String,
    pub m: usize,
    pub n: usize,
    pub description: String,
    pub assumptions: Vec<String>,
    pub is_default: bool,
}

impl QuorumPolicy {
    pub fn custom(m: usize, n: usize) -> Self {
        QuorumPolicy {
            id: format!("custom-{m}-of-{n}"),
            label: format!("{m}-of-{n} (custom)"),
            m,
            n,
            description: "Reviewer-defined quorum. Not established by the review draft.".into(),
            assumptions: vec!["Every guardian key is independently controlled.".into()],
            is_default: false,
        }
    }
}

/// Illustrative quorum presets. None is a requirement of the review draft.
pub fn quorum_presets() -> Vec<QuorumPolicy> {
    vec![
        QuorumPolicy {
            id: "demo-2-of-3".into(),
            label: "2-of-3 (demonstration preset)".into(),
            m: 2,
            n: 3,
            description: "Majority of three guardians. A convenient demo default, not a protocol requirement established by the review draft.".into(),
            assumptions: vec!["Three independent guardian keys.".into(), "Any two guardians can be reached within the decision window.".into()],
            is_default: true,
        },
        QuorumPolicy {
            id: "unanimous-3-of-3".into(),
            label: "3-of-3 (unanimous)".into(),
            m: 3,
            n: 3,
            description: "Every guardian must sign. Slowest to reach; one unavailable guardian blocks every change.".into(),
            assumptions: vec!["All guardians reachable.".into()],
            is_default: false,
        },
        QuorumPolicy {
            id: "lighter-1-of-3".into(),
            label: "1-of-3 (lighter path, hypothetical)".into(),
            m: 1,
            n: 3,
            description: "A single guardian suffices. Shown to explore whether 'technically simple' changes should take a lighter path (review draft, case B open question). Not endorsed.".into(),
            assumptions: vec!["One guardian's judgment is sufficient for the change class this is applied to.".into()],
            is_default: false,
        },
        QuorumPolicy {
            id: "wide-3-of-5".into(),
            label: "3-of-5".into(),
            m: 3,
            n: 5,
            description: "Majority of five. Requires five local test keys; the demo generates them.".into(),
            assumptions: vec!["Five independent guardian keys.".into()],
            is_default: false,
        },
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approve,
    Reject,
    RequestChanges,
}

impl Decision {
    pub fn label(&self) -> &'static str {
        match self {
            Decision::Approve => "approve",
            Decision::Reject => "reject",
            Decision::RequestChanges => "request_changes",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub guardian_id: String,
    pub proposal_id: String,
    pub revision: u32,
    pub decision: Decision,
    /// The commitment the guardian recomputed and signed. For approvals this
    /// is "exactly H"; nothing else is approved.
    pub commitment: Commitment,
    pub note: String,
    pub surface_mode_viewed: SurfaceMode,
    pub signature_hex: String,
}

/// The exact bytes a guardian signs.
pub fn approval_message(
    proposal_id: &str,
    revision: u32,
    decision: Decision,
    commitment: &Commitment,
) -> Vec<u8> {
    format!(
        "{APPROVAL_DOMAIN}\n{proposal_id}\n{revision}\n{}\n{}",
        decision.label(),
        commitment.hex()
    )
    .into_bytes()
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalError {
    #[error("guardian {0} is not in the guardian set")]
    UnknownGuardian(String),
    #[error("guardian {0} has no local test key; the demo cannot sign for a real guardian")]
    NoSigningKey(String),
    #[error("invalid key material: {0}")]
    Key(String),
    #[error("signature of guardian {0} does not verify: {1}")]
    BadSignature(String, String),
}

#[allow(clippy::too_many_arguments)]
pub fn sign_approval(
    set: &GuardianSet,
    guardian_id: &str,
    proposal_id: &str,
    revision: u32,
    decision: Decision,
    commitment: Commitment,
    note: &str,
    mode: SurfaceMode,
) -> Result<Approval, ApprovalError> {
    let g = set
        .get(guardian_id)
        .ok_or_else(|| ApprovalError::UnknownGuardian(guardian_id.into()))?;
    if g.test_secret_hex.is_none() {
        return Err(ApprovalError::NoSigningKey(guardian_id.into()));
    }
    let sk = g.signing_key().map_err(ApprovalError::Key)?;
    let msg = approval_message(proposal_id, revision, decision, &commitment);
    let sig: Signature = sk.sign(&msg);
    Ok(Approval {
        guardian_id: guardian_id.into(),
        proposal_id: proposal_id.into(),
        revision,
        decision,
        commitment,
        note: note.into(),
        surface_mode_viewed: mode,
        signature_hex: hex::encode(sig.to_bytes()),
    })
}

/// Verify one approval against the guardian set. Uses the guardian's public
/// key only; never the test secret.
pub fn verify_approval(set: &GuardianSet, a: &Approval) -> Result<(), ApprovalError> {
    let g = set
        .get(&a.guardian_id)
        .ok_or_else(|| ApprovalError::UnknownGuardian(a.guardian_id.clone()))?;
    let vk = g.verifying_key().map_err(ApprovalError::Key)?;
    let sig_bytes = hex::decode(&a.signature_hex)
        .map_err(|e| ApprovalError::BadSignature(a.guardian_id.clone(), e.to_string()))?;
    let arr: [u8; 64] = sig_bytes.try_into().map_err(|_| {
        ApprovalError::BadSignature(a.guardian_id.clone(), "signature must be 64 bytes".into())
    })?;
    let sig = Signature::from_bytes(&arr);
    let msg = approval_message(&a.proposal_id, a.revision, a.decision, &a.commitment);
    vk.verify(&msg, &sig)
        .map_err(|e| ApprovalError::BadSignature(a.guardian_id.clone(), e.to_string()))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authorization {
    pub proposal_id: String,
    pub revision: u32,
    pub commitment: Commitment,
    pub policy: QuorumPolicy,
    pub approvals: Vec<Approval>,
    pub issued_seq: u64,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuorumError {
    #[error("quorum not met: {have} valid approvals of exactly H, {need} required")]
    NotMet { have: usize, need: usize },
    #[error("a guardian rejected the proposal: {0}")]
    Rejected(String),
    #[error("approval error: {0}")]
    Approval(#[from] ApprovalError),
}

/// Evaluate approvals for a specific proposal revision and commitment.
/// Only approvals whose signature verifies, whose decision is `Approve`, and
/// whose commitment equals `commitment` count. Duplicate guardians count once.
pub fn evaluate_quorum(
    set: &GuardianSet,
    policy: &QuorumPolicy,
    approvals: &[Approval],
    proposal_id: &str,
    revision: u32,
    commitment: Commitment,
    issued_seq: u64,
) -> Result<Authorization, QuorumError> {
    let mut counted: Vec<Approval> = Vec::new();
    for a in approvals {
        if a.proposal_id != proposal_id || a.revision != revision {
            continue;
        }
        verify_approval(set, a)?;
        if a.decision == Decision::Reject {
            return Err(QuorumError::Rejected(a.guardian_id.clone()));
        }
        if a.decision == Decision::Approve
            && a.commitment == commitment
            && !counted.iter().any(|c| c.guardian_id == a.guardian_id)
        {
            counted.push(a.clone());
        }
    }
    if counted.len() < policy.m {
        return Err(QuorumError::NotMet {
            have: counted.len(),
            need: policy.m,
        });
    }
    Ok(Authorization {
        proposal_id: proposal_id.into(),
        revision,
        commitment,
        policy: policy.clone(),
        approvals: counted,
        issued_seq,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::compute_commitment;

    #[test]
    fn local_keys_are_deterministic() {
        let a = GuardianSet::local_test(3);
        let b = GuardianSet::local_test(3);
        assert_eq!(a, b);
        assert_eq!(a.guardians[0].id, "guardian-1");
        assert!(a
            .public()
            .guardians
            .iter()
            .all(|g| g.test_secret_hex.is_none()));
    }

    #[test]
    fn signatures_verify_and_bind_commitment() {
        let set = GuardianSet::local_test(3);
        let h = compute_commitment(b"m", b"p");
        let a = sign_approval(
            &set,
            "guardian-1",
            "A-1",
            1,
            Decision::Approve,
            h,
            "",
            SurfaceMode::Structured,
        )
        .unwrap();
        verify_approval(&set, &a).unwrap();
        let mut tampered = a.clone();
        tampered.commitment = compute_commitment(b"m", b"q");
        assert!(verify_approval(&set, &tampered).is_err());
        let mut wrong_decision = a.clone();
        wrong_decision.decision = Decision::Reject;
        assert!(verify_approval(&set, &wrong_decision).is_err());
        // Public set verifies without secrets, cannot sign.
        verify_approval(&set.public(), &a).unwrap();
        assert!(matches!(
            sign_approval(
                &set.public(),
                "guardian-1",
                "A-1",
                1,
                Decision::Approve,
                h,
                "",
                SurfaceMode::Minimal
            ),
            Err(ApprovalError::NoSigningKey(_))
        ));
    }

    #[test]
    fn quorum_counts_only_exact_h() {
        let set = GuardianSet::local_test(3);
        let policy = quorum_presets().into_iter().find(|p| p.is_default).unwrap();
        let h = compute_commitment(b"m", b"p");
        let other = compute_commitment(b"m", b"x");
        let a1 = sign_approval(
            &set,
            "guardian-1",
            "A-1",
            1,
            Decision::Approve,
            h,
            "",
            SurfaceMode::Structured,
        )
        .unwrap();
        let a2_other = sign_approval(
            &set,
            "guardian-2",
            "A-1",
            1,
            Decision::Approve,
            other,
            "",
            SurfaceMode::Structured,
        )
        .unwrap();
        assert!(matches!(
            evaluate_quorum(&set, &policy, &[a1.clone(), a2_other], "A-1", 1, h, 1),
            Err(QuorumError::NotMet { have: 1, need: 2 })
        ));
        let a2 = sign_approval(
            &set,
            "guardian-2",
            "A-1",
            1,
            Decision::Approve,
            h,
            "",
            SurfaceMode::Full,
        )
        .unwrap();
        let auth = evaluate_quorum(
            &set,
            &policy,
            &[a1.clone(), a1.clone(), a2.clone()],
            "A-1",
            1,
            h,
            1,
        )
        .unwrap();
        assert_eq!(auth.approvals.len(), 2);
        // Old revision approvals do not carry over.
        assert!(evaluate_quorum(&set, &policy, &[a1, a2], "A-1", 2, h, 1).is_err());
    }
}
