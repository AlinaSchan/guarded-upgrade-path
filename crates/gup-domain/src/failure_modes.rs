//! The six failure modes from page 4, as executable regression scenarios.
//!
//! Each runs through the real engine on a fresh workspace and reports what
//! was expected, what was observed and whether the expected behaviour held.
//! They do not steer the reviewer to a conclusion; they show what the model
//! does in each case.

use crate::engine::{
    AgentStep, CandidateVariant, ExecutionOutcome, ProposalStatus, TamperVariant, Workspace,
};
use crate::evidence::Classification;
use crate::executor::ExecutionRefusal;
use crate::guardians::Decision;
use crate::incident::{IncidentParams, Rung};
use crate::surface::SurfaceMode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureModeId {
    ProofHoldsPropertyWrong,
    ChecksPassJudgmentStops,
    ValidCodeUnwantedEconomics,
    RollbackThatIsnt,
    ReviewedIsNotExecuted,
    QuorumLateAuthorityCreeps,
}

impl FailureModeId {
    pub fn all() -> [FailureModeId; 6] {
        [
            FailureModeId::ProofHoldsPropertyWrong,
            FailureModeId::ChecksPassJudgmentStops,
            FailureModeId::ValidCodeUnwantedEconomics,
            FailureModeId::RollbackThatIsnt,
            FailureModeId::ReviewedIsNotExecuted,
            FailureModeId::QuorumLateAuthorityCreeps,
        ]
    }
    pub fn key(&self) -> &'static str {
        match self {
            FailureModeId::ProofHoldsPropertyWrong => "F1",
            FailureModeId::ChecksPassJudgmentStops => "F2",
            FailureModeId::ValidCodeUnwantedEconomics => "F3",
            FailureModeId::RollbackThatIsnt => "F4",
            FailureModeId::ReviewedIsNotExecuted => "F5",
            FailureModeId::QuorumLateAuthorityCreeps => "F6",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::all()
            .into_iter()
            .find(|f| f.key().eq_ignore_ascii_case(s.trim()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureModeInfo {
    pub id: FailureModeId,
    pub key: String,
    pub title: String,
    pub description: String,
    pub needs: String,
    pub reference: Option<String>,
    pub scenario: String,
    pub question: u8,
}

pub fn info(id: FailureModeId) -> FailureModeInfo {
    let (title, description, needs, reference, scenario, question) = match id {
        FailureModeId::ProofHoldsPropertyWrong => (
            "The proof holds; the property was wrong",
            "A checker confirms exactly what it was asked. An incomplete specification, or one the proposing agent has learned to satisfy, passes cleanly and reads as assurance.",
            "Guardians see which properties were checked, not only that they passed.",
            None,
            "A",
            1,
        ),
        FailureModeId::ChecksPassJudgmentStops => (
            "Checks pass, judgment stops",
            "A clean summary invites signatures without reading. The decision drifts to whoever chose the checks, which may be the proposer.",
            "Every package states what was not checked and what is left to guardians.",
            None,
            "A",
            2,
        ),
        FailureModeId::ValidCodeUnwantedEconomics => (
            "Valid code, unwanted economics",
            "A parameter or logic change can keep every technical invariant intact and still move value between users or shift risk onto them.",
            "Economic effect as its own row, decided by guardians rather than inferred from passing checks.",
            None,
            "B",
            4,
        ),
        FailureModeId::RollbackThatIsnt => (
            "A rollback that isn't",
            "After a migration the old implementation may misread new state, and whatever users did under the new version stays done.",
            "Reversibility established before approval; recovery treated as a fresh decision.",
            None,
            "C",
            3,
        ),
        FailureModeId::ReviewedIsNotExecuted => (
            "Reviewed is not executed",
            "Guardians approve one artifact and another one runs. If the proposer also renders the summary, a hash only relocates the trust.",
            "Guardians recompute the hash and key checks with tooling independent of the proposer.",
            Some("cf. Bybit, Feb 2025: signers approved what a compromised interface displayed."),
            "A",
            1,
        ),
        FailureModeId::QuorumLateAuthorityCreeps => (
            "Quorum arrives late, or authority creeps",
            "A fix can wait days for signatures while the fault is exploited. Standing emergency power for the agent removes the wait by moving approval authority without a vote.",
            "Any pre-authorized action stays narrower than the decision it buys time for.",
            Some("cf. Compound, 2021: a fix needed seven days of review, vote and timelock."),
            "D",
            5,
        ),
    };
    FailureModeInfo {
        id,
        key: id.key().into(),
        title: title.into(),
        description: description.into(),
        needs: needs.into(),
        reference: reference.map(|s| s.to_string()),
        scenario: scenario.into(),
        question,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureModeResult {
    pub id: FailureModeId,
    pub key: String,
    pub title: String,
    pub expected: String,
    pub observed: String,
    /// The expected behaviour of the model was observed.
    pub holds: bool,
    pub steps: Vec<String>,
    pub evidence: Value,
}

fn result(
    id: FailureModeId,
    expected: &str,
    observed: String,
    holds: bool,
    steps: Vec<String>,
    evidence: Value,
) -> FailureModeResult {
    let i = info(id);
    FailureModeResult {
        id,
        key: i.key,
        title: i.title,
        expected: expected.into(),
        observed,
        holds,
        steps,
        evidence,
    }
}

fn approve_two(ws: &mut Workspace, key: &str) {
    ws.guardian_decide(key, "guardian-1", Decision::Approve, "")
        .expect("sign");
    ws.guardian_decide(key, "guardian-2", Decision::Approve, "")
        .expect("sign");
}

/// Run a failure mode through the engine on a fresh workspace.
pub fn run(id: FailureModeId) -> FailureModeResult {
    let mut ws = Workspace::new();
    let mut steps = Vec::new();
    match id {
        FailureModeId::ProofHoldsPropertyWrong => {
            ws.set_candidate_variant("A", CandidateVariant::FlatAmount)
                .unwrap();
            steps.push("Candidate for case A replaced by a variant that pays a flat amount (tier weight dropped).".into());
            ws.agent_run_to_submission("A").unwrap();
            let inst = ws.scenario("A").unwrap();
            let pkg = inst.package.as_ref().unwrap();
            let rb = inst.replay.as_ref().unwrap();
            let all_checks_pass = pkg.technical_verdict.all_checked_passed;
            let fuzz_clean = rb.fuzz_candidate.violations == 0;
            let boundary_names_amount = pkg
                .specification_boundary
                .iter()
                .any(|b| b.to_lowercase().contains("amount"));
            let invariants_not_checked = pkg
                .items
                .iter()
                .find(|i| i.row == "verified invariants")
                .map(|i| {
                    i.not_checked
                        .iter()
                        .any(|n| n.to_lowercase().contains("amount"))
                })
                .unwrap_or(false);
            // Ground truth outside the package: amounts differ from the correct fix.
            let correct = crate::simulation::replay(
                &inst.genesis,
                &inst.stream,
                Some(&crate::fixtures::distribution_bundle_2_3_1()),
            )
            .0;
            let amounts_wrong = correct.total_gross != rb.candidate.total_gross;
            steps.push(format!(
                "Deterministic checks: {} passed; fuzz uniqueness violations: {}.",
                if all_checks_pass { "all" } else { "not all" },
                rb.fuzz_candidate.violations
            ));
            steps.push(format!("Package claims complete correctness: {}. Specification boundary names payout amounts: {}.", pkg.claims_complete_correctness, boundary_names_amount));
            steps.push(format!("Ground truth (outside the package): candidate pays {} units vs {} for the correct fix.", rb.candidate.total_gross, correct.total_gross));
            let holds = all_checks_pass
                && fuzz_clean
                && boundary_names_amount
                && invariants_not_checked
                && !pkg.claims_complete_correctness
                && amounts_wrong;
            result(
                id,
                "The checks pass because their specification omits the payout amount. The package exposes that boundary and cannot claim complete correctness.",
                "Checks passed and the uniqueness property held, yet the candidate pays the wrong amounts. The package lists 'payout amount correctness' as not checked and claims_complete_correctness = false.".to_string(),
                holds,
                steps,
                json!({"all_checks_pass": all_checks_pass, "fuzz_violations": rb.fuzz_candidate.violations, "specification_boundary": pkg.specification_boundary, "candidate_total_gross": rb.candidate.total_gross, "correct_total_gross": correct.total_gross, "claims_complete_correctness": pkg.claims_complete_correctness}),
            )
        }
        FailureModeId::ChecksPassJudgmentStops => {
            ws.agent_run_to_submission("A").unwrap();
            let cmp = ws.compare_surfaces("A").unwrap();
            let minimal = &cmp.views[0];
            let structured = &cmp.views[1];
            let assumptions_section = structured
                .sections
                .iter()
                .find(|s| s.title == "Assumptions")
                .map(|s| s.item_ids.len())
                .unwrap_or(0);
            let judgment_rows = ws
                .scenario("A")
                .unwrap()
                .package
                .as_ref()
                .unwrap()
                .judgments()
                .len();
            steps.push(format!(
                "Mode A headline: \"{}\" with {} omissions listed.",
                minimal.headline,
                minimal.omitted.len()
            ));
            steps.push(format!("Mode B shows {} assumption rows and {} judgment rows; H identical across modes: {}.", assumptions_section, judgment_rows, cmp.commitment_identical));
            let holds = minimal.items_shown == 0
                && minimal.omitted.len() >= 3
                && assumptions_section >= 1
                && judgment_rows >= 1
                && cmp.commitment_identical
                && cmp.package_unchanged;
            result(
                id,
                "The minimal surface states what it omits; the structured surface makes assumptions and judgment rows visible; neither changes the proposal or H.",
                format!("Mode A shows {} of {} rows and lists {} omitted categories. Mode B exposes {} assumptions and {} judgment rows. The commitment is identical in every mode.", minimal.items_shown, minimal.items_total, minimal.omitted.len(), assumptions_section, judgment_rows),
                holds,
                steps,
                json!({"minimal_omitted": minimal.omitted, "structured_sections": structured.sections.iter().map(|s| s.title.clone()).collect::<Vec<_>>(), "commitment_identical": cmp.commitment_identical}),
            )
        }
        FailureModeId::ValidCodeUnwantedEconomics => {
            ws.agent_run_to_submission("B").unwrap();
            let inst = ws.scenario("B").unwrap();
            let pkg = inst.package.as_ref().unwrap();
            let econ = inst.economics.as_ref().unwrap();
            let econ_item = pkg
                .items
                .iter()
                .find(|i| i.row == "economic impact")
                .unwrap();
            let is_judgment = econ_item
                .classifications
                .contains(&Classification::Judgment)
                && !econ_item.classifications.contains(&Classification::Checked);
            let verdict_silent_on_desirability = !pkg
                .technical_verdict
                .statement
                .to_lowercase()
                .contains("desirable")
                || pkg
                    .technical_verdict
                    .statement
                    .contains("not that the change is correct or desirable");
            steps.push(format!(
                "Technical verdict: {}",
                pkg.technical_verdict.statement
            ));
            steps.push(format!(
                "Economic row classifications: {:?}.",
                econ_item.classifications
            ));
            steps.push(format!(
                "Economic comparison: {} rows; treasury fee {} → {}.",
                econ.rows.len(),
                econ.rows.last().map(|r| r.before).unwrap_or(0),
                econ.rows.last().map(|r| r.after).unwrap_or(0)
            ));
            let holds = pkg.technical_verdict.all_checked_passed
                && is_judgment
                && verdict_silent_on_desirability
                && econ.rows.iter().any(|r| r.delta != 0);
            result(
                id,
                "Every technical check passes, value moves between users and the treasury, and the economic effect stays a separate guardian judgment.",
                format!("All checks passed. The economic row is tagged {:?}; the verdict states that passing checks does not establish desirability. Net payouts fall for every tier while treasury income rises.", econ_item.classifications),
                holds,
                steps,
                json!({"technical_verdict": pkg.technical_verdict, "economic_rows": econ.rows, "desirability": pkg.desirability}),
            )
        }
        FailureModeId::RollbackThatIsnt => {
            ws.agent_run_to_submission("C").unwrap();
            approve_two(&mut ws, "C");
            let applied = matches!(ws.execute("C").unwrap(), ExecutionOutcome::Applied { .. });
            steps.push(format!("3.0.0 executed with migration: {applied}."));
            let users = ws.simulate_users_act("C", 2).unwrap();
            steps.push(format!("Users act under 3.0.0: {users}"));
            let attempt = ws.attempt_code_revert("C").unwrap();
            let l2 = attempt
                .assessment
                .layers
                .iter()
                .find(|l| l.layer == 2)
                .map(|l| l.status)
                .unwrap();
            let l3 = attempt
                .assessment
                .layers
                .iter()
                .find(|l| l.layer == 3)
                .map(|l| l.status)
                .unwrap();
            steps.push(format!("Code revert attempt blocked: {}. Layer 2: {:?}; layer 3: {:?}; misreads if reverted: {}.", attempt.blocked, l2, l3, attempt.assessment.misreads_if_reverted));
            let holds = applied
                && attempt.blocked
                && l2 == crate::recovery::LayerStatus::Blocked
                && l3 == crate::recovery::LayerStatus::Partial
                && attempt.assessment.misreads_if_reverted > 0
                && attempt.assessment.requires_new_authorization;
            result(
                id,
                "After the migration and user activity, reverting code is blocked: the old code misreads the new state and the external effects stand; recovery requires a new decision.",
                format!("Automatic revert blocked. Layer 2 (state compatibility) is {:?} with {} misreads in the probe; layer 3 lists effects that reverting cannot undo; layer 4 requires {} new privileged operations.", l2, attempt.assessment.misreads_if_reverted, attempt.assessment.actions.len()),
                holds,
                steps,
                json!({"assessment": attempt.assessment}),
            )
        }
        FailureModeId::ReviewedIsNotExecuted => {
            ws.agent_run_to_submission("A").unwrap();
            approve_two(&mut ws, "A");
            let approved_h = ws
                .scenario("A")
                .unwrap()
                .authorization
                .as_ref()
                .unwrap()
                .commitment;
            let mut refusals = Vec::new();
            let mut summaries_unchanged = true;
            let mut verifier_rejects = true;
            for v in [
                TamperVariant::DivertFee,
                TamperVariant::RerouteSelector,
                TamperVariant::SilentParameter,
                TamperVariant::ManifestAndPayload,
            ] {
                let rec = ws.tamper("A", v).unwrap();
                summaries_unchanged &= ws
                    .scenario("A")
                    .unwrap()
                    .displayed_summary
                    .as_ref()
                    .unwrap()
                    .claimed_commitment
                    == approved_h;
                let out = ws.execute("A").unwrap();
                let refused = matches!(
                    out,
                    ExecutionOutcome::Refused {
                        refusal: ExecutionRefusal::CommitmentMismatch { .. }
                    }
                );
                let rep = ws.verify_independently("A").unwrap();
                verifier_rejects &= rep.verdict == crate::verifier::Verdict::Reject;
                refusals.push(refused);
                steps.push(format!(
                    "{:?}: {} — executor refused: {}; verifier: {:?}; state still {}.",
                    v,
                    rec.description,
                    refused,
                    rep.verdict,
                    ws.scenario("A")
                        .unwrap()
                        .state
                        .distribution_package()
                        .version
                ));
                ws.restore_candidate("A").unwrap();
            }
            let restored_ok = matches!(ws.execute("A").unwrap(), ExecutionOutcome::Applied { .. });
            steps.push(format!(
                "Original artifact restored and executed: {restored_ok}."
            ));
            let holds = refusals.iter().all(|r| *r)
                && summaries_unchanged
                && verifier_rejects
                && restored_ok;
            result(
                id,
                "A substituted artifact is refused by the executor and detected by the independent verifier while the agent's displayed summary still shows the approved upgrade.",
                format!("{} substitution variants refused with a commitment mismatch; the displayed summary never changed; the verifier rejected each; the original artifact then executed.", refusals.iter().filter(|r| **r).count()),
                holds,
                steps,
                json!({"approved_commitment": approved_h.hex(), "variants_refused": refusals}),
            )
        }
        FailureModeId::QuorumLateAuthorityCreeps => {
            let params = IncidentParams::default();
            let outcomes = ws.incident_set_params("D", params).unwrap();
            let normal = outcomes
                .iter()
                .find(|o| o.policy.id == "normal-approval")
                .unwrap()
                .clone();
            let limit = outcomes
                .iter()
                .find(|o| o.policy.id == "pre-limit-function")
                .unwrap()
                .clone();
            let pause_all = outcomes
                .iter()
                .find(|o| o.policy.id == "pre-pause-protocol")
                .unwrap()
                .clone();
            steps.push(format!(
                "Normal approval: loss {} before the fix at block {}; nothing interrupted.",
                normal.loss_before_containment, normal.fix_block
            ));
            steps.push(format!(
                "Limit one function: loss {}; {} legitimate batch claims interrupted.",
                limit.loss_before_containment, limit.interrupted.batch_claims
            ));
            steps.push(format!(
                "Pause everything: loss {}; {} exits and {} claims interrupted.",
                pause_all.loss_before_containment,
                pause_all.interrupted.exits,
                pause_all.interrupted.single_claims + pause_all.interrupted.batch_claims
            ));
            let refused_default = ws
                .incident_agent_request("D", Rung::LimitOneFunction)
                .is_err();
            ws.incident_set_policy("D", "pre-limit-function").unwrap();
            let refused_above = ws.incident_agent_request("D", Rung::PauseProtocol).is_err();
            let granted = ws
                .incident_agent_request("D", Rung::LimitOneFunction)
                .is_ok();
            let lift_refused = ws.incident_agent_lift("D").is_err();
            ws.advance_blocks("D", 400).unwrap();
            let expired = ws.incident_check_grant("D").unwrap().is_err();
            ws.incident_guardians_lift("D").unwrap();
            ws.incident_close("D").unwrap();
            let closed_refused = ws.incident_agent_request("D", Rung::Flag).is_err();
            steps.push(format!("Agent under default policy refused: {refused_default}; above-rung request refused: {refused_above}; in-rung granted: {granted}; agent lift refused: {lift_refused}; grant expired after 400 blocks: {expired}; after close, even 'flag' refused: {closed_refused}."));
            let holds = normal.loss_before_containment > limit.loss_before_containment
                && limit.interrupted.exits == 0
                && pause_all.interrupted.exits > 0
                && refused_default
                && refused_above
                && granted
                && lift_refused
                && expired
                && closed_refused;
            result(
                id,
                "Waiting for quorum costs loss; acting without quorum costs interrupted activity; any pre-authorized power is narrower than the upgrade decision, bounded, and never standing.",
                format!("Loss until fix: {} (normal) vs {} (limit one function). Pausing everything interrupts {} exits. The agent could not exceed its rung, lift its own containment, keep an expired grant, or act after the incident closed.", normal.loss_before_containment, limit.loss_before_containment, pause_all.interrupted.exits),
                holds,
                steps,
                json!({"outcomes": outcomes.iter().map(|o| json!({"policy": o.policy.id, "loss": o.loss_before_containment, "interrupted": o.interrupted})).collect::<Vec<_>>()}),
            )
        }
    }
}

pub fn run_all() -> Vec<FailureModeResult> {
    FailureModeId::all().into_iter().map(run).collect()
}

#[allow(dead_code)]
fn _unused(_: ProposalStatus, _: AgentStep, _: SurfaceMode) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_failure_mode_behaves_as_expected() {
        for r in run_all() {
            assert!(
                r.holds,
                "{} did not hold: {}\n{:#?}",
                r.key, r.observed, r.steps
            );
        }
    }
}
