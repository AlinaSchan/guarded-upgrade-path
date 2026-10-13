//! The constrained executor.
//!
//! Given an authorization that refers to `H` and an artifact to execute, the
//! executor independently recomputes the artifact's commitment, verifies the
//! guardian signatures and the quorum, and only then applies the payload it
//! was handed. It never modifies or substitutes the payload, never consults
//! the agent's displayed summary, and escalates an unexpected post-state back
//! to the guardians instead of acting on its own.

use crate::artifact::Artifact;
use crate::canonical::Commitment;
use crate::checks::{structural_checks, CheckResult};
use crate::guardians::{Authorization, GuardianSet};
use crate::protocol::{InstallReport, ProtocolState, DISTRIBUTION};
use crate::simulation::{build_stream, replay};
use crate::verifier::{verify, Verdict, VerifierReport};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionRefusal {
    /// The artifact handed over does not hash to the approved H.
    CommitmentMismatch {
        expected: Commitment,
        computed: Commitment,
        verifier: VerifierReport,
    },
    /// Signatures or quorum did not verify over H.
    AuthorizationInvalid { verifier: VerifierReport },
    /// The authorization is for a different proposal or revision.
    WrongProposal { expected: String, got: String },
    /// The payload cannot be interpreted by the local model.
    PayloadRejected { reason: String },
}

impl ExecutionRefusal {
    pub fn summary(&self) -> String {
        match self {
            ExecutionRefusal::CommitmentMismatch { expected, computed, .. } => format!("Refused: the artifact handed to the executor hashes to {} but the guardians approved {}. Nothing was applied.", computed.short(), expected.short()),
            ExecutionRefusal::AuthorizationInvalid { verifier } => format!("Refused: authorization did not verify ({}). Nothing was applied.", verifier.reasons.join("; ")),
            ExecutionRefusal::WrongProposal { expected, got } => format!("Refused: authorization refers to {expected}, execution requested for {got}."),
            ExecutionRefusal::PayloadRejected { reason } => format!("Refused: payload rejected by the local model: {reason}."),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostStateVerification {
    pub checks: Vec<CheckResult>,
    pub smoke_replay: String,
    pub ok: bool,
    /// Set when the post-state is unexpected. Returning to the guardians is a
    /// new decision, not a rollback.
    pub escalation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub proposal_id: String,
    pub commitment: Commitment,
    pub verifier: VerifierReport,
    pub install: InstallReport,
    pub block: u64,
    pub post_state: PostStateVerification,
    pub statement: String,
}

/// Execute an authorized artifact against the protocol state.
#[allow(clippy::result_large_err)]
pub fn execute(
    authorization: &Authorization,
    guardians: &GuardianSet,
    artifact: &Artifact,
    proposal_id: &str,
    state: &mut ProtocolState,
) -> Result<ExecutionReport, ExecutionRefusal> {
    if authorization.proposal_id != proposal_id {
        return Err(ExecutionRefusal::WrongProposal {
            expected: authorization.proposal_id.clone(),
            got: proposal_id.into(),
        });
    }
    // 1-2. Independent verification of the artifact as handed over. The
    // manifest is re-serialized here from the struct the executor holds; the
    // verifier canonicalizes it again and recomputes H from the payload bytes.
    let manifest_json =
        serde_json::to_vec(&artifact.manifest).map_err(|e| ExecutionRefusal::PayloadRejected {
            reason: e.to_string(),
        })?;
    let report = verify(
        &manifest_json,
        &artifact.payload,
        Some(authorization.commitment),
        Some(authorization),
        Some(&guardians.public()),
    );
    match report.computed_commitment {
        Some(c) if c != authorization.commitment => {
            return Err(ExecutionRefusal::CommitmentMismatch {
                expected: authorization.commitment,
                computed: c,
                verifier: report,
            });
        }
        None => return Err(ExecutionRefusal::AuthorizationInvalid { verifier: report }),
        _ => {}
    }
    if report.verdict != Verdict::Accept {
        return Err(ExecutionRefusal::AuthorizationInvalid { verifier: report });
    }
    // 3. Apply only the authorized payload.
    let bundle = artifact
        .bundle()
        .map_err(|e| ExecutionRefusal::PayloadRejected {
            reason: e.to_string(),
        })?;
    let before = state.distribution_package().clone();
    let install = state
        .install(&bundle, proposal_id, authorization.commitment)
        .map_err(|e| ExecutionRefusal::PayloadRejected {
            reason: e.to_string(),
        })?;

    // 5. Post-state verification: the installed package must equal the payload
    // and a short smoke replay must hold the uniqueness invariant.
    let installed = state.distribution_package();
    let mut checks = Vec::new();
    let installed_matches = installed.installed == bundle && installed.version == bundle.version;
    checks.push(CheckResult {
        id: "post.installed_equals_payload".into(),
        name: "Installed implementation equals the authorized payload".into(),
        passed: installed_matches,
        classification: crate::evidence::Classification::Checked,
        method: "Exact comparison of the installed bundle with the payload that was executed."
            .into(),
        scope: format!("{DISTRIBUTION} package"),
        output: format!(
            "installed {} ({} functions)",
            installed.version,
            installed.functions.len()
        ),
        not_covered: vec!["Behaviour of the installed code beyond the smoke replay.".into()],
    });
    let layout_ok = installed.storage == bundle.storage
        && state.distribution.format_version == bundle.storage.format_version;
    checks.push(CheckResult {
        id: "post.storage_format".into(),
        name: "Persistent state format matches the installed layout".into(),
        passed: layout_ok,
        classification: crate::evidence::Classification::Checked,
        method:
            "Comparison of the state's format version with the installed layout's declared format."
                .into(),
        scope: "Distribution persistent state".into(),
        output: format!(
            "state format {}, layout format {}",
            state.distribution.format_version, bundle.storage.format_version
        ),
        not_covered: vec!["Whether every migrated value kept its meaning.".into()],
    });
    let roles_ok =
        installed.installed.roles.clone().normalized() == state.roles.clone().normalized();
    checks.push(CheckResult {
        id: "post.roles".into(),
        name: "Roles unchanged by execution".into(),
        passed: roles_ok,
        classification: crate::evidence::Classification::Checked,
        method: "Set comparison of protocol roles before and after.".into(),
        scope: "upgrade admin and guardian set".into(),
        output: format!("{:?}", state.roles),
        not_covered: vec![],
    });
    let structural = structural_checks(&before, &state.roles, install.format_version, &bundle);
    let structural_ok = structural
        .iter()
        .filter(|c| c.id != "storage.layout" || bundle.migration.is_none())
        .all(|c| c.passed);
    checks.extend(structural);

    // Smoke replay: 2 periods, 300 claims, random repeat claims and keeper lag.
    let stream = build_stream(0xC0FFEE ^ state.block, 2, 300, &[1], 3, false);
    let (rep, _) = replay(state, &stream, None);
    let smoke_ok = rep.duplicates.is_empty() && rep.misreads == 0 && rep.paid > 0;
    let smoke = format!("{} claims replayed on a copy of the post-state with {} ({}): {} paid, {} rejected, {} duplicates, {} misreads", rep.claims, rep.version, rep.impl_id, rep.paid, rep.rejected, rep.duplicates.len(), rep.misreads);

    let ok = installed_matches && layout_ok && roles_ok && structural_ok && smoke_ok;
    let escalation = if ok {
        None
    } else {
        Some(format!(
            "Unexpected post-state: installed_matches={installed_matches}, layout_ok={layout_ok}, roles_ok={roles_ok}, structural_ok={structural_ok}, smoke_ok={smoke_ok}. Returned to the guardians for a new decision; the executor took no further action."
        ))
    };
    let post_state = PostStateVerification {
        checks,
        smoke_replay: smoke,
        ok,
        escalation,
    };
    let statement = if ok {
        format!(
            "Applied exactly {} ({} -> {}) at block {}. Post-state verification passed.",
            authorization.commitment.short(),
            install.from_version,
            install.to_version,
            state.block
        )
    } else {
        format!(
            "Applied exactly {} ({} -> {}) at block {}. Post-state verification FAILED; escalated.",
            authorization.commitment.short(),
            install.from_version,
            install.to_version,
            state.block
        )
    };
    Ok(ExecutionReport {
        proposal_id: proposal_id.into(),
        commitment: authorization.commitment,
        verifier: report,
        install,
        block: state.block,
        post_state,
        statement,
    })
}
