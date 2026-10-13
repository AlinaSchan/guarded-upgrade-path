//! Recovery model: four layers instead of "rollback".
//!
//! 1. Code — can the prior implementation be restored?
//! 2. Persistent state — can the prior implementation read the state as it now is?
//! 3. External effects — what has already happened under the new version?
//! 4. Recovery — which further operations reach an acceptable state, and who authorizes them?
//!
//! "Rollback" usually covers layer 1. Layers 2-4 must be shown. Reverting
//! code alone is never treated as a recovery, and automatic recovery is
//! blocked whenever layer 2 or layer 4 says more is needed.

use crate::artifact::{Artifact, ChangeKind, ImplementationBundle};
use crate::fixtures;
use crate::protocol::{EffectKind, ProtocolState, UpgradeRecord};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerStatus {
    Possible,
    Blocked,
    Partial,
    Nothing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerResult {
    pub layer: u8,
    pub title: String,
    pub question: String,
    pub status: LayerStatus,
    pub finding: String,
    pub details: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryAction {
    pub description: String,
    pub privileged: bool,
    pub authorized_by: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryAssessment {
    pub upgrade: Option<String>,
    pub layers: Vec<LayerResult>,
    pub automatic_recovery_blocked: bool,
    pub code_revert_is_recovery: bool,
    pub why_code_revert_insufficient: Vec<String>,
    pub requires_new_authorization: bool,
    pub actions: Vec<RecoveryAction>,
    pub misreads_if_reverted: u64,
    pub statement: String,
}

fn layer(
    n: u8,
    title: &str,
    question: &str,
    status: LayerStatus,
    finding: impl Into<String>,
    details: Vec<String>,
) -> LayerResult {
    LayerResult {
        layer: n,
        title: title.into(),
        question: question.into(),
        status,
        finding: finding.into(),
        details,
    }
}

/// Assess recovery from the most recent upgrade recorded in `state`.
pub fn assess(state: &ProtocolState) -> RecoveryAssessment {
    let Some(last) = state.history.last() else {
        return RecoveryAssessment {
            upgrade: None,
            layers: vec![
                layer(1, "Code rollback", "Can the prior implementation be restored?", LayerStatus::Nothing, "No upgrade has been applied in this scenario.", vec![]),
                layer(2, "State compatibility", "Can the prior code read the state as it now is?", LayerStatus::Nothing, "Nothing to assess.", vec![]),
                layer(3, "External effects", "What transfers, permissions, integrations and user actions already happened?", LayerStatus::Nothing, "Nothing to assess.", vec![]),
                layer(4, "Recovery", "Which further operations reach an acceptable state, and who must authorize them?", LayerStatus::Nothing, "Nothing to assess.", vec![]),
            ],
            automatic_recovery_blocked: false,
            code_revert_is_recovery: false,
            why_code_revert_insufficient: vec![],
            requires_new_authorization: false,
            actions: vec![],
            misreads_if_reverted: 0,
            statement: "No upgrade applied; nothing to recover from.".into(),
        };
    };
    assess_upgrade(state, last)
}

pub fn assess_upgrade(state: &ProtocolState, last: &UpgradeRecord) -> RecoveryAssessment {
    let prior = &last.prior_bundle;
    let current_format = state.distribution.format_version;

    // Layer 1: code
    let l1 = layer(
        1,
        "Code rollback",
        "Can the prior implementation be restored?",
        LayerStatus::Possible,
        format!("The prior implementation ({} {}) is recorded and can be re-proposed as an artifact.", prior.package, prior.version),
        vec![
            format!("Prior bundle: {} functions, storage format {}.", prior.functions.len(), prior.storage.format_version),
            "Restoring code is itself an upgrade: it needs a new artifact, a new H and a new approval.".into(),
        ],
    );

    // Layer 2: state compatibility — would the prior code read the state as it now is?
    let mut misreads = 0u64;
    let compatible = prior.storage.format_version == current_format;
    let l2 = if compatible {
        layer(
            2,
            "State compatibility",
            "Can the prior code read the state as it now is?",
            LayerStatus::Possible,
            format!("The state is still format {}; the prior implementation declares the same layout.", current_format),
            vec!["A layout match does not show that every value still means what the prior code assumes (e.g. what lastPaidPeriod denotes).".into()],
        )
    } else {
        // Demonstrate: run the prior reader against the migrated state.
        let mut probe = state.clone();
        probe.distribution_package_mut().functions = prior
            .functions
            .iter()
            .map(|f| (f.selector.clone(), f.entry()))
            .collect();
        probe.distribution_package_mut().installed = prior.clone();
        let probe_block = state.block + state.period_length;
        let wallets: Vec<u32> = state
            .distribution
            .claim_records
            .keys()
            .take(200)
            .copied()
            .collect();
        let mut sample = 0u64;
        for w in &wallets {
            sample += 1;
            let _ = probe.claim(*w, probe_block);
        }
        misreads = probe.distribution.misreads;
        layer(
            2,
            "State compatibility",
            "Can the prior code read the state as it now is?",
            LayerStatus::Blocked,
            format!("No. The state is format {current_format}; the prior implementation reads format {}. Probing {sample} migrated wallets with the prior reader produced {misreads} misreads.", prior.storage.format_version),
            vec![
                "Slot 1 now holds packed ClaimRecords; the prior code interprets it as a uint64 lastPaidPeriod.".into(),
                "A layout check of the prior code against its own declared layout would pass while the old code misreads the new data.".into(),
            ],
        )
    };

    // Layer 3: external effects since the upgrade block
    let effects: Vec<_> = state
        .effects
        .iter()
        .filter(|e| e.block >= last.block)
        .collect();
    let paid_since: i64 = state
        .distribution
        .per_period
        .values()
        .filter(|p| p.period >= state.period_of(last.block))
        .map(|p| p.gross)
        .sum();
    let claims_since: u64 = state
        .distribution
        .per_period
        .values()
        .filter(|p| p.period >= state.period_of(last.block))
        .map(|p| p.paid)
        .sum();
    let mut l3_details: Vec<String> = effects
        .iter()
        .map(|e| {
            format!(
                "{:?} @ block {}: {} (count {}{})",
                e.kind,
                e.block,
                e.description,
                e.count,
                e.amount
                    .map(|a| format!(", amount {a}"))
                    .unwrap_or_default()
            )
        })
        .collect();
    if claims_since > 0 {
        l3_details.push(format!("{claims_since} payouts totalling {paid_since} units were made under the new version. Reverting code does not return them."));
    }
    let has_effects = !effects.is_empty() || claims_since > 0;
    let l3 = layer(
        3,
        "External effects",
        "What transfers, permissions, integrations and user actions already happened?",
        if has_effects {
            LayerStatus::Partial
        } else {
            LayerStatus::Nothing
        },
        if has_effects {
            format!("{} external effects and {} payouts occurred under the new version. None of them is undone by reinstalling code.", effects.len(), claims_since)
        } else {
            "No external effects have been recorded since the upgrade yet.".into()
        },
        l3_details,
    );

    // Layer 4: recovery operations and who authorizes them
    let mut actions = Vec::new();
    let mut why = Vec::new();
    match last
        .prior_bundle
        .migration
        .as_ref()
        .map(|_| ())
        .or(if compatible { None } else { Some(()) })
    {
        Some(()) if !compatible => {
            actions.push(RecoveryAction { description: format!("Reverse migration: transform format {} state back to format {} (a new artifact with its own H).", current_format, prior.storage.format_version), privileged: true, authorized_by: "guardian quorum via a new proposal".into() });
            why.push("The prior code cannot read the migrated state; reinstalling it would misread every migrated wallet.".into());
        }
        _ => {}
    }
    if claims_since > 0 {
        actions.push(RecoveryAction { description: format!("Decide what to do about {claims_since} payouts ({paid_since} units) made under the new version: leave them, reconcile them, or claw them back. Each is a separate decision."), privileged: true, authorized_by: "guardian judgment; not part of any code revert".into() });
        why.push("Payouts made under the new version stay paid whatever code is installed.".into());
    }
    if effects.iter().any(|e| e.kind == EffectKind::Integration) {
        why.push(
            "Integrations that adapted to the new version will not adapt back automatically."
                .into(),
        );
    }
    if let ChangeKind::ImplementationOnly = infer_change_kind(prior, state) {
        why.push("Reinstalling the prior implementation restores its behaviour, including the defect the upgrade fixed.".into());
    }
    let l4 = layer(
        4,
        "Recovery",
        "Which further operations reach an acceptable state, and who must authorize them?",
        if actions.is_empty() {
            LayerStatus::Possible
        } else {
            LayerStatus::Blocked
        },
        if actions.is_empty() {
            "Restoring the prior code is the only operation; it still requires a new proposal and approval.".to_string()
        } else {
            format!(
                "{} further privileged operations are required; each needs its own authorization.",
                actions.len()
            )
        },
        actions
            .iter()
            .map(|a| format!("{} — authorized by {}", a.description, a.authorized_by))
            .collect(),
    );

    let automatic_blocked = !compatible || !actions.is_empty();
    let statement = if automatic_blocked {
        "Reverting code alone is not a recovery here. Automatic recovery is blocked; the required operations must be proposed and authorized as new decisions.".to_string()
    } else {
        "Reverting code would restore the prior behaviour, but it is still a new upgrade with its own H and approval, and it does not undo anything that already happened.".to_string()
    };
    RecoveryAssessment {
        upgrade: Some(format!(
            "{} {} -> {} ({})",
            last.package,
            last.from_version,
            last.to_version,
            last.commitment.short()
        )),
        layers: vec![l1, l2, l3, l4],
        automatic_recovery_blocked: automatic_blocked,
        code_revert_is_recovery: false,
        why_code_revert_insufficient: why,
        requires_new_authorization: true,
        actions,
        misreads_if_reverted: misreads,
        statement,
    }
}

fn infer_change_kind(prior: &ImplementationBundle, state: &ProtocolState) -> ChangeKind {
    let cur = &state.distribution_package().installed;
    if prior.storage.format_version != cur.storage.format_version {
        ChangeKind::StateMigration
    } else if prior.functions == cur.functions && prior.parameters != cur.parameters {
        ChangeKind::ParameterOnly
    } else {
        ChangeKind::ImplementationOnly
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryError {
    #[error("automatic code revert blocked: {0}")]
    Blocked(String),
    #[error("nothing to recover from")]
    NothingApplied,
}

/// Attempt an automatic "revert the code only". Allowed to *draft* a revert
/// proposal only when layer 2 is compatible and no further privileged action
/// is required; otherwise blocked. Never applies anything by itself.
pub fn attempt_code_revert(
    state: &ProtocolState,
) -> Result<(RecoveryAssessment, Artifact), RecoveryError> {
    let last = state.history.last().ok_or(RecoveryError::NothingApplied)?;
    let assessment = assess_upgrade(state, last);
    if assessment.automatic_recovery_blocked {
        return Err(RecoveryError::Blocked(
            assessment.why_code_revert_insufficient.join(" "),
        ));
    }
    let artifact = Artifact::from_bundle(
        &last.prior_bundle,
        &last.to_version,
        ChangeKind::Recovery,
        &format!("Reinstall {} {} (code revert). Restores prior behaviour; does not undo effects since block {}.", last.package, last.from_version, last.block),
        "proposal-agent (simulated)",
    )
    .expect("fixture bundle canonicalizes");
    Ok((assessment, artifact))
}

/// Draft the recovery proposal for a blocked case (Case C): a reverse migration.
pub fn draft_recovery_proposal(state: &ProtocolState) -> Result<Artifact, RecoveryError> {
    let last = state.history.last().ok_or(RecoveryError::NothingApplied)?;
    if state.distribution.format_version == 2 {
        let bundle = fixtures::distribution_bundle_3_0_1_reverse();
        Artifact::from_bundle(
            &bundle,
            &last.to_version,
            ChangeKind::Recovery,
            "Recovery proposal: reverse migration format 2 -> 1 and reinstall format-1 claim logic. Payouts made under 3.0.0 remain paid; a separate decision covers them.",
            "proposal-agent (simulated)",
        )
        .map_err(|e| RecoveryError::Blocked(e.to_string()))
    } else {
        let (_, a) = attempt_code_revert(state)?;
        Ok(a)
    }
}
