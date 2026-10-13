//! Presentation views: what the desktop UI receives. Heavy fields (claim
//! streams, genesis states, per-wallet maps) are left out; everything the
//! reviewer needs to see is here, derived from the engine, never computed by
//! the frontend.

use crate::artifact::ArtifactManifest;
use crate::checks::CheckResult;
use crate::engine::{
    CompletedProposal, Detection, DisplayedSummary, EconomicComparison, ExecutionOutcome,
    IncidentState, Observation, ProposalStatus, RevertAttempt, ScenarioInstance, Settings,
    TamperRecord, Workspace,
};
use crate::evidence::EvidencePackage;
use crate::failure_modes::{info, FailureModeId, FailureModeInfo, FailureModeResult};
use crate::guardians::{quorum_presets, Approval, Authorization, GuardianSet, QuorumPolicy};
use crate::incident::{incident_policies, IncidentPolicy, Rung};
use crate::notes::ReviewNote;
use crate::protocol::{DuplicatePayout, ExternalEffect, PeriodStat};
use crate::questions::{questions, ReviewQuestion};
use crate::recovery::RecoveryAssessment;
use crate::scenarios::ScenarioMeta;
use crate::simulation::{FuzzReport, ReplayReport};
use crate::surface::SurfaceMode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeSummary {
    pub proposal_id: String,
    pub from_version: String,
    pub to_version: String,
    pub commitment: String,
    pub commitment_short: String,
    pub block: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSummary {
    pub block: u64,
    pub period: u64,
    pub version: String,
    pub format_version: u32,
    pub fee_bps: i64,
    pub paused: bool,
    pub pause_detail: String,
    pub payout_count: u64,
    pub duplicates: usize,
    pub treasury: i64,
    pub fee_collected: i64,
    pub diverted: i64,
    pub wallets_with_history: usize,
    pub effects: Vec<ExternalEffect>,
    pub history: Vec<UpgradeSummary>,
    pub functions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactView {
    pub manifest: ArtifactManifest,
    pub manifest_json: String,
    pub payload_text: String,
    pub payload_len: usize,
    pub payload_sha256: String,
    pub commitment: String,
    pub commitment_short: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayStats {
    pub version: String,
    pub impl_id: String,
    pub stream: String,
    pub claims: u64,
    pub paid: u64,
    pub rejected: u64,
    pub misreads: u64,
    pub duplicate_count: usize,
    pub duplicates: Vec<DuplicatePayout>,
    pub affected_wallets: usize,
    pub total_gross: i64,
    pub total_net: i64,
    pub total_fee: i64,
    pub diverted: i64,
    pub overpayment_gross: i64,
    pub overpayment_share_bps: i64,
    pub average_daily_gross: i64,
    pub per_period: Vec<PeriodStat>,
    pub first_block: u64,
    pub last_block: u64,
    pub migrated_entries: u64,
}

impl ReplayStats {
    fn from(r: &ReplayReport) -> Self {
        ReplayStats {
            version: r.version.clone(),
            impl_id: r.impl_id.clone(),
            stream: r.stream.clone(),
            claims: r.claims,
            paid: r.paid,
            rejected: r.rejected,
            misreads: r.misreads,
            duplicate_count: r.duplicates.len(),
            duplicates: r.duplicates.iter().take(60).cloned().collect(),
            affected_wallets: r.affected_wallets.len(),
            total_gross: r.total_gross,
            total_net: r.total_net,
            total_fee: r.total_fee,
            diverted: r.diverted,
            overpayment_gross: r.overpayment_gross,
            overpayment_share_bps: r.overpayment_share_bps(),
            average_daily_gross: r.average_daily_gross(),
            per_period: r.per_period.clone(),
            first_block: r.first_block,
            last_block: r.last_block,
            migrated_entries: r.migrated_entries,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaySummary {
    pub narrative: String,
    pub production: ReplayStats,
    pub candidate: ReplayStats,
    pub fuzz_current: FuzzReport,
    pub fuzz_candidate: FuzzReport,
    pub differing_wallets: usize,
    pub stream_description: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioView {
    pub key: String,
    pub meta: ScenarioMeta,
    pub proposal_id: String,
    pub revision: u32,
    pub status: ProposalStatus,
    pub status_label: String,
    pub stage: String,
    pub state: StateSummary,
    pub observation: Option<Observation>,
    pub detection: Option<Detection>,
    pub diagnosis: Option<String>,
    pub candidate: Option<ArtifactView>,
    pub candidate_diff: Option<String>,
    pub displayed_summary: Option<DisplayedSummary>,
    pub checks: Vec<CheckResult>,
    pub replay: Option<ReplaySummary>,
    pub economics: Option<EconomicComparison>,
    pub package: Option<EvidencePackage>,
    pub surface_mode: SurfaceMode,
    pub guardians: GuardianSet,
    pub quorum_policy: QuorumPolicy,
    pub approvals: Vec<Approval>,
    pub invalidated_approvals: usize,
    pub authorization: Option<Authorization>,
    pub execution: Option<ExecutionOutcome>,
    pub tamper: Option<TamperRecord>,
    pub recovery: Option<RecoveryAssessment>,
    pub revert_attempt: Option<RevertAttempt>,
    pub incident: Option<IncidentState>,
    pub completed: Vec<CompletedProposal>,
    pub narrative: Vec<String>,
    pub next_agent_step: Option<String>,
}

fn next_step(status: ProposalStatus) -> Option<&'static str> {
    use ProposalStatus::*;
    Some(match status {
        Idle => "observe",
        Observed => "detect",
        Detected => "diagnose",
        Diagnosed => "construct",
        Constructed => "check",
        Checked => "simulate",
        Simulated => "assemble",
        Assembled => "commit",
        Committed => "submit",
        ChangesRequested => "revise",
        _ => return None,
    })
}

pub fn scenario_view(inst: &ScenarioInstance) -> ScenarioView {
    let s = &inst.state;
    let d = &s.distribution;
    let pkg = s.distribution_package();
    let state = StateSummary {
        block: s.block,
        period: s.period_of(s.block),
        version: pkg.version.clone(),
        format_version: d.format_version,
        fee_bps: s.fee_bps(),
        paused: s.pause.any(),
        pause_detail: if s.pause.protocol {
            "entire protocol paused".into()
        } else if !s.pause.packages.is_empty() {
            format!("packages paused: {:?}", s.pause.packages)
        } else if !s.pause.functions.is_empty() {
            format!("functions limited: {:?}", s.pause.functions)
        } else {
            "not paused".into()
        },
        payout_count: d.payout_count,
        duplicates: d.duplicates.len(),
        treasury: d.treasury,
        fee_collected: d.fee_collected,
        diverted: d.diverted,
        wallets_with_history: d.last_paid_period.len() + d.claim_records.len(),
        effects: s.effects.clone(),
        history: s
            .history
            .iter()
            .map(|h| UpgradeSummary {
                proposal_id: h.proposal_id.clone(),
                from_version: h.from_version.clone(),
                to_version: h.to_version.clone(),
                commitment: h.commitment.hex(),
                commitment_short: h.commitment.short(),
                block: h.block,
            })
            .collect(),
        functions: pkg
            .functions
            .values()
            .map(|f| {
                format!(
                    "{} {} → {}{}",
                    f.selector,
                    f.name,
                    f.impl_id,
                    if f.privileged { " [privileged]" } else { "" }
                )
            })
            .collect(),
    };
    let candidate = inst.candidate.as_ref().map(|a| {
        let h = a.commitment().expect("canonical");
        ArtifactView {
            manifest: a.manifest.clone(),
            manifest_json: a.manifest_pretty(),
            payload_text: a.payload_utf8(),
            payload_len: a.payload.len(),
            payload_sha256: a.payload_sha256(),
            commitment: h.hex(),
            commitment_short: h.short(),
        }
    });
    let replay = inst.replay.as_ref().map(|r| ReplaySummary {
        narrative: r.narrative.clone(),
        production: ReplayStats::from(&r.production),
        candidate: ReplayStats::from(&r.candidate),
        fuzz_current: r.fuzz_current.clone(),
        fuzz_candidate: r.fuzz_candidate.clone(),
        differing_wallets: r.differing_wallets.len(),
        stream_description: inst.stream.description.clone(),
    });
    ScenarioView {
        key: inst.meta.key.clone(),
        meta: inst.meta.clone(),
        proposal_id: inst.proposal_id.clone(),
        revision: inst.revision,
        status: inst.status,
        status_label: inst.status.label().into(),
        stage: inst.status.stage().into(),
        state,
        observation: inst.observation.clone(),
        detection: inst.detection.clone(),
        diagnosis: inst.diagnosis.clone(),
        candidate,
        candidate_diff: inst.candidate_diff.clone(),
        displayed_summary: inst.displayed_summary.clone(),
        checks: inst.checks.clone(),
        replay,
        economics: inst.economics.clone(),
        package: inst.package.clone(),
        surface_mode: inst.surface_mode,
        guardians: inst.guardians.public(),
        quorum_policy: inst.quorum_policy.clone(),
        approvals: inst.approvals.clone(),
        invalidated_approvals: inst.invalidated_approvals.len(),
        authorization: inst.authorization.clone(),
        execution: inst.execution.clone(),
        tamper: inst.tamper.clone(),
        recovery: inst.recovery.clone(),
        revert_attempt: inst.revert_attempt.clone(),
        incident: inst.incident.clone(),
        completed: inst.completed.clone(),
        narrative: inst.narrative.clone(),
        next_agent_step: next_step(inst.status).map(|s| s.to_string()),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioSummary {
    pub key: String,
    pub title: String,
    pub package_title: String,
    pub status: ProposalStatus,
    pub status_label: String,
    pub stage: String,
    pub commitment_short: Option<String>,
    pub tampered: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RungInfo {
    pub rung: Rung,
    pub number: u8,
    pub label: String,
    pub changes_protocol_state: bool,
    pub side_effects: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceView {
    pub app_version: String,
    pub scope_statement: String,
    pub scenarios: Vec<ScenarioSummary>,
    pub notes: Vec<ReviewNote>,
    pub audit: Vec<crate::engine::AuditEvent>,
    pub failure_modes: Vec<FailureModeInfo>,
    pub failure_results: Vec<FailureModeResult>,
    pub questions: Vec<ReviewQuestion>,
    pub quorum_presets: Vec<QuorumPolicy>,
    pub incident_policies: Vec<IncidentPolicy>,
    pub rungs: Vec<RungInfo>,
    pub settings: Settings,
    pub hypothesis: Vec<String>,
    pub assumptions: Vec<String>,
}

pub fn workspace_view(ws: &Workspace) -> WorkspaceView {
    WorkspaceView {
        app_version: crate::version().into(),
        scope_statement: crate::scope_statement().into(),
        scenarios: ws
            .scenarios
            .values()
            .map(|i| ScenarioSummary {
                key: i.meta.key.clone(),
                title: i.meta.title.clone(),
                package_title: i.meta.package_title.clone(),
                status: i.status,
                status_label: i.status.label().into(),
                stage: i.status.stage().into(),
                commitment_short: i.current_commitment().map(|c| c.short()),
                tampered: i.tamper.is_some(),
            })
            .collect(),
        notes: ws.notes.clone(),
        audit: ws.audit.iter().rev().take(300).cloned().collect(),
        failure_modes: FailureModeId::all().into_iter().map(info).collect(),
        failure_results: ws.failure_results.values().cloned().collect(),
        questions: questions(),
        quorum_presets: quorum_presets(),
        incident_policies: incident_policies(),
        rungs: Rung::all().iter().enumerate().map(|(i, r)| RungInfo { rung: *r, number: i as u8 + 1, label: r.label().into(), changes_protocol_state: r.changes_protocol_state(), side_effects: r.side_effects().into() }).collect(),
        settings: ws.settings.clone(),
        hypothesis: vec![
            "An autonomous agent can observe conditions, prepare a candidate upgrade, verify selected properties, simulate execution and assemble evidence.".into(),
            "The decision to change the protocol remains with the designated human guardians.".into(),
            "The executor is authorized to apply only the precise artifact approved by the guardians.".into(),
            "The decision only holds if the payload executed is the one the guardians reviewed.".into(),
        ],
        assumptions: vec![
            "Guardian keys are independently controlled (simulated here with local test keys).".into(),
            "The checks' specification is complete enough to be meaningful (failure mode 1 tests this).".into(),
            "Guardians read what the surface shows them (failure mode 2 tests this).".into(),
            "The executor's verifier is independent of the proposer (the CLI runs out of process).".into(),
            "Recovery is a new decision, not a rollback (failure mode 4 tests this).".into(),
            "Emergency powers, if any, stay narrower than the decision they buy time for (failure mode 6 tests this).".into(),
        ],
    }
}
