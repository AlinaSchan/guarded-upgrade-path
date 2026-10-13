//! The application engine: scenario instances, the proposal state machine,
//! guardian and executor actions, recovery and incident actions, the audit
//! log and the persisted workspace.
//!
//! Every authority boundary is enforced here, in Rust. The frontend only
//! asks; the engine decides and records.

use crate::artifact::Artifact;
use crate::canonical::Commitment;
use crate::checks::CheckResult;
use crate::evidence::EvidencePackage;
use crate::executor::{execute, ExecutionRefusal, ExecutionReport};
use crate::guardians::{
    evaluate_quorum, quorum_presets, sign_approval, Approval, ApprovalError, Authorization,
    Decision, GuardianSet, QuorumError, QuorumPolicy,
};
use crate::incident::{
    agent_attempt_lift, agent_request_action, check_grant, compare_policies, incident_policies,
    AuthorityRefusal, EmergencyGrant, IncidentOutcome, IncidentParams, IncidentPolicy, Rung,
};
use crate::notes::ReviewNote;
use crate::protocol::{EffectKind, ProtocolState, DISTRIBUTION};
use crate::recovery::{
    assess, attempt_code_revert, draft_recovery_proposal, RecoveryAssessment, RecoveryError,
};
use crate::scenarios::{self, ScenarioId, ScenarioMeta};
use crate::simulation::{
    build_stream, run_stream, shift_stream, ClaimStream, FuzzReport, ReplayReport,
};
use crate::surface::{compare, project, SurfaceComparison, SurfaceMode, SurfaceView};
use crate::verifier::{verify, VerifierReport};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const WORKSPACE_VERSION: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Idle,
    Observed,
    Detected,
    Diagnosed,
    Constructed,
    Checked,
    Simulated,
    Assembled,
    Committed,
    Submitted,
    UnderReview,
    ChangesRequested,
    Rejected,
    Approved,
    ExecutionRefused,
    Executed,
    Escalated,
}

impl ProposalStatus {
    pub fn stage(&self) -> &'static str {
        use ProposalStatus::*;
        match self {
            Idle | Observed | Detected | Diagnosed | Constructed | Checked | Simulated
            | Assembled | Committed => "proposal",
            Submitted | UnderReview | ChangesRequested | Rejected | Approved => "approval",
            ExecutionRefused | Executed | Escalated => "execution",
        }
    }
    pub fn label(&self) -> &'static str {
        use ProposalStatus::*;
        match self {
            Idle => "idle",
            Observed => "observed",
            Detected => "condition detected",
            Diagnosed => "diagnosed",
            Constructed => "candidate constructed",
            Checked => "checks run",
            Simulated => "simulation run",
            Assembled => "evidence assembled",
            Committed => "committed to H",
            Submitted => "submitted for review",
            UnderReview => "under guardian review",
            ChangesRequested => "changes requested",
            Rejected => "rejected",
            Approved => "approved (quorum over H)",
            ExecutionRefused => "execution refused",
            Executed => "executed",
            Escalated => "escalated to guardians",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStep {
    Observe,
    Detect,
    Diagnose,
    Construct,
    Check,
    Simulate,
    Assemble,
    Commit,
    Submit,
    Revise,
}

impl AgentStep {
    pub fn pipeline() -> [AgentStep; 9] {
        [
            AgentStep::Observe,
            AgentStep::Detect,
            AgentStep::Diagnose,
            AgentStep::Construct,
            AgentStep::Check,
            AgentStep::Simulate,
            AgentStep::Assemble,
            AgentStep::Commit,
            AgentStep::Submit,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CandidateVariant {
    #[default]
    Default,
    /// Failure mode 1: passes the checked properties, pays the wrong amount.
    FlatAmount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TamperVariant {
    DivertFee,
    RerouteSelector,
    SilentParameter,
    ManifestAndPayload,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signal {
    pub name: String,
    pub value: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub block: u64,
    pub period: u64,
    pub package_version: String,
    pub signals: Vec<Signal>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detection {
    pub title: String,
    pub query: String,
    pub block_range: (u64, u64),
    pub result: String,
    pub affected_wallets: Vec<u32>,
    pub rows: Vec<String>,
}

/// What the proposing agent's own interface renders. Kept apart from the
/// artifact so that substitution attacks can leave it untouched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayedSummary {
    pub title: String,
    pub summary: String,
    pub lines: Vec<String>,
    pub claimed_commitment: Commitment,
    pub rendered_by: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayBundle {
    pub production: ReplayReport,
    pub candidate: ReplayReport,
    pub fuzz_current: FuzzReport,
    pub fuzz_candidate: FuzzReport,
    pub differing_wallets: Vec<u32>,
    pub narrative: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconRow {
    pub group: String,
    pub before: i64,
    pub after: i64,
    pub delta: i64,
    pub delta_bps: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicComparison {
    pub parameter: String,
    pub before: i64,
    pub after: i64,
    pub periods: u64,
    pub rows: Vec<EconRow>,
    pub transfers_under_new_value: u64,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TamperRecord {
    pub variant: TamperVariant,
    pub description: String,
    pub after_status: ProposalStatus,
    pub original_commitment: Commitment,
    pub tampered_commitment: Commitment,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ExecutionOutcome {
    Applied { report: ExecutionReport },
    Refused { refusal: ExecutionRefusal },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecomputeResult {
    pub computed: Commitment,
    pub claimed: Commitment,
    pub matches: bool,
    pub manifest_canonical_len: usize,
    pub payload_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardianOutcome {
    pub approval: Approval,
    pub status: ProposalStatus,
    pub authorization: Option<Authorization>,
    pub quorum_note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevertAttempt {
    pub blocked: bool,
    pub reason: String,
    pub assessment: RecoveryAssessment,
    pub draft: Option<Artifact>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentState {
    pub incident_id: String,
    pub open: bool,
    pub policy_id: String,
    pub params: IncidentParams,
    pub outcomes: Vec<IncidentOutcome>,
    pub grant: Option<EmergencyGrant>,
    pub log: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletedProposal {
    pub proposal_id: String,
    pub commitment: Option<Commitment>,
    pub status: ProposalStatus,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioInstance {
    pub id: ScenarioId,
    pub meta: ScenarioMeta,
    pub proposal_id: String,
    pub proposal_counter: u32,
    pub revision: u32,
    pub status: ProposalStatus,
    /// Rebuilt deterministically on load; never persisted.
    #[serde(skip, default = "default_state")]
    pub genesis: ProtocolState,
    pub state: ProtocolState,
    /// Rebuilt deterministically on load; never persisted.
    #[serde(skip, default = "default_stream")]
    pub stream: ClaimStream,
    pub candidate_variant: CandidateVariant,
    pub observation: Option<Observation>,
    pub detection: Option<Detection>,
    pub diagnosis: Option<String>,
    pub candidate: Option<Artifact>,
    pub original_candidate: Option<Artifact>,
    pub candidate_diff: Option<String>,
    pub displayed_summary: Option<DisplayedSummary>,
    pub checks: Vec<CheckResult>,
    pub replay: Option<ReplayBundle>,
    pub economics: Option<EconomicComparison>,
    pub package: Option<EvidencePackage>,
    pub surface_mode: SurfaceMode,
    pub guardians: GuardianSet,
    pub quorum_policy: QuorumPolicy,
    pub approvals: Vec<Approval>,
    pub invalidated_approvals: Vec<Approval>,
    pub authorization: Option<Authorization>,
    pub execution: Option<ExecutionOutcome>,
    pub tamper: Option<TamperRecord>,
    pub recovery: Option<RecoveryAssessment>,
    pub revert_attempt: Option<RevertAttempt>,
    pub incident: Option<IncidentState>,
    pub completed: Vec<CompletedProposal>,
    pub narrative: Vec<String>,
}

fn default_state() -> ProtocolState {
    scenarios::genesis(ScenarioId::CaseA)
}

fn default_stream() -> ClaimStream {
    ClaimStream {
        name: String::new(),
        seed: 0,
        periods: 0,
        period_length: 1_000,
        first_block: 0,
        last_block: 0,
        claim_count: 0,
        description: String::new(),
        keeper_lag: Default::default(),
        lag_window_wallets: Default::default(),
        events: vec![],
    }
}

impl ScenarioInstance {
    /// Restore the fields that are not persisted.
    pub fn rehydrate(&mut self) {
        self.genesis = scenarios::genesis(self.id);
        self.stream = scenarios::stream(self.id);
    }

    pub fn new(id: ScenarioId) -> Self {
        let meta = scenarios::meta(id);
        let genesis = scenarios::genesis(id);
        let state = scenarios::live(id, &genesis);
        let stream = scenarios::stream(id);
        let policy = quorum_presets()
            .into_iter()
            .find(|p| p.is_default)
            .expect("default preset");
        let incident = if id == ScenarioId::CaseD {
            let params = IncidentParams::default();
            Some(IncidentState {
                incident_id: "INC-1".into(),
                open: true,
                policy_id: incident_policies()
                    .into_iter()
                    .find(|p| p.is_default)
                    .map(|p| p.id)
                    .unwrap_or_default(),
                outcomes: compare_policies(&params),
                params,
                grant: None,
                log: vec![
                    "Incident INC-1 opened: claimFor repeated-wallet drain in progress.".into(),
                ],
            })
        } else {
            None
        };
        ScenarioInstance {
            id,
            meta: meta.clone(),
            proposal_id: format!("{}-1", meta.key),
            proposal_counter: 1,
            revision: 1,
            status: ProposalStatus::Idle,
            genesis,
            state,
            stream,
            candidate_variant: CandidateVariant::Default,
            observation: None,
            detection: None,
            diagnosis: None,
            candidate: None,
            original_candidate: None,
            candidate_diff: None,
            displayed_summary: None,
            checks: vec![],
            replay: None,
            economics: None,
            package: None,
            surface_mode: SurfaceMode::Structured,
            guardians: GuardianSet::local_test(policy.n),
            quorum_policy: policy,
            approvals: vec![],
            invalidated_approvals: vec![],
            authorization: None,
            execution: None,
            tamper: None,
            recovery: None,
            revert_attempt: None,
            incident,
            completed: vec![],
            narrative: vec![],
        }
    }

    fn note(&mut self, line: impl Into<String>) {
        self.narrative.push(line.into());
    }

    pub fn current_commitment(&self) -> Option<Commitment> {
        self.candidate.as_ref().and_then(|a| a.commitment().ok())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub seq: u64,
    pub scenario: Option<String>,
    pub actor: String,
    pub action: String,
    pub detail: String,
    pub commitment: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Settings {
    pub dark_mode: bool,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineError {
    #[error("unknown scenario {0}")]
    UnknownScenario(String),
    #[error("{action} is not allowed while the proposal is {status:?}")]
    InvalidTransition {
        action: String,
        status: ProposalStatus,
    },
    #[error("{0}")]
    Quorum(#[from] QuorumError),
    #[error("{0}")]
    Approval(#[from] ApprovalError),
    #[error("{0}")]
    Recovery(#[from] RecoveryError),
    #[error("{0}")]
    Authority(#[from] AuthorityRefusal),
    #[error("{0}")]
    Other(String),
}

pub type EngineResult<T> = Result<T, EngineError>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub version: u32,
    pub seq: u64,
    pub scenarios: BTreeMap<String, ScenarioInstance>,
    pub audit: Vec<AuditEvent>,
    pub notes: Vec<ReviewNote>,
    pub failure_results: BTreeMap<String, crate::failure_modes::FailureModeResult>,
    pub settings: Settings,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    pub fn new() -> Self {
        let mut scenarios = BTreeMap::new();
        for id in ScenarioId::all() {
            scenarios.insert(id.key().to_string(), ScenarioInstance::new(id));
        }
        let mut ws = Workspace {
            version: WORKSPACE_VERSION,
            seq: 0,
            scenarios,
            audit: vec![],
            notes: vec![],
            failure_results: BTreeMap::new(),
            settings: Settings::default(),
        };
        ws.audit(
            None,
            "system",
            "workspace created",
            "Four scenarios built from deterministic fixtures.",
            None,
        );
        ws
    }

    /// Deserialize a persisted workspace and rebuild its derived fields.
    pub fn from_json(json: &str) -> Result<Workspace, String> {
        let mut ws: Workspace = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if ws.version != WORKSPACE_VERSION {
            return Err(format!(
                "workspace version {} does not match {}",
                ws.version, WORKSPACE_VERSION
            ));
        }
        for inst in ws.scenarios.values_mut() {
            inst.rehydrate();
        }
        Ok(ws)
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    pub fn audit(
        &mut self,
        scenario: Option<&str>,
        actor: &str,
        action: &str,
        detail: impl Into<String>,
        commitment: Option<Commitment>,
    ) {
        self.seq += 1;
        self.audit.push(AuditEvent {
            seq: self.seq,
            scenario: scenario.map(|s| s.to_string()),
            actor: actor.into(),
            action: action.into(),
            detail: detail.into(),
            commitment: commitment.map(|c| c.hex()),
            timestamp: None,
        });
    }

    pub fn scenario(&self, key: &str) -> EngineResult<&ScenarioInstance> {
        let id = ScenarioId::parse(key).ok_or_else(|| EngineError::UnknownScenario(key.into()))?;
        self.scenarios
            .get(id.key())
            .ok_or_else(|| EngineError::UnknownScenario(key.into()))
    }

    fn scenario_mut(&mut self, key: &str) -> EngineResult<&mut ScenarioInstance> {
        let id = ScenarioId::parse(key).ok_or_else(|| EngineError::UnknownScenario(key.into()))?;
        self.scenarios
            .get_mut(id.key())
            .ok_or_else(|| EngineError::UnknownScenario(key.into()))
    }

    pub fn reset_scenario(&mut self, key: &str) -> EngineResult<()> {
        let id = ScenarioId::parse(key).ok_or_else(|| EngineError::UnknownScenario(key.into()))?;
        self.scenarios
            .insert(id.key().to_string(), ScenarioInstance::new(id));
        self.audit(
            Some(id.key()),
            "reviewer",
            "scenario reset",
            "Rebuilt deterministically from fixtures.",
            None,
        );
        Ok(())
    }

    pub fn reset_all(&mut self) {
        for id in ScenarioId::all() {
            let _ = self.reset_scenario(id.key());
        }
    }

    pub fn set_candidate_variant(&mut self, key: &str, v: CandidateVariant) -> EngineResult<()> {
        let inst = self.scenario_mut(key)?;
        if inst.candidate.is_some() {
            return Err(EngineError::InvalidTransition {
                action: "set candidate variant".into(),
                status: inst.status,
            });
        }
        inst.candidate_variant = v;
        Ok(())
    }

    // ----------------------------------------------------------------- Stage A

    pub fn agent_step(&mut self, key: &str, step: AgentStep) -> EngineResult<ProposalStatus> {
        use ProposalStatus as S;
        let (detail, commitment, status) = {
            let inst = self.scenario_mut(key)?;
            let invalid = |inst: &ScenarioInstance| EngineError::InvalidTransition {
                action: format!("{step:?}"),
                status: inst.status,
            };
            match step {
                AgentStep::Observe => {
                    if inst.status != S::Idle {
                        return Err(invalid(inst));
                    }
                    let o = scenarios::observe(inst);
                    let d = format!(
                        "Observed block {} (period {}), {} {}; {} signals.",
                        o.block,
                        o.period,
                        DISTRIBUTION,
                        o.package_version,
                        o.signals.len()
                    );
                    inst.observation = Some(o);
                    inst.status = S::Observed;
                    inst.note(d.clone());
                    (d, None, inst.status)
                }
                AgentStep::Detect => {
                    if inst.status != S::Observed {
                        return Err(invalid(inst));
                    }
                    let d = scenarios::detect(inst);
                    let line = format!("{}: {}", d.title, d.result);
                    inst.detection = Some(d);
                    inst.status = S::Detected;
                    inst.note(line.clone());
                    (line, None, inst.status)
                }
                AgentStep::Diagnose => {
                    if inst.status != S::Detected {
                        return Err(invalid(inst));
                    }
                    let d = scenarios::diagnose(inst);
                    inst.diagnosis = Some(d.clone());
                    inst.status = S::Diagnosed;
                    inst.note("Diagnosis recorded.");
                    (d, None, inst.status)
                }
                AgentStep::Construct => {
                    if inst.status != S::Diagnosed {
                        return Err(invalid(inst));
                    }
                    let (a, diff, shown) = scenarios::construct(inst, inst.candidate_variant);
                    let h = a.commitment().expect("canonical");
                    let d = format!(
                        "Candidate {} constructed ({} payload bytes). Agent-computed H = {}.",
                        a.manifest.to_version,
                        a.payload.len(),
                        h.short()
                    );
                    inst.candidate = Some(a);
                    inst.original_candidate = None;
                    inst.candidate_diff = Some(diff);
                    inst.displayed_summary = Some(shown);
                    inst.checks.clear();
                    inst.replay = None;
                    inst.economics = None;
                    inst.package = None;
                    inst.tamper = None;
                    inst.status = S::Constructed;
                    inst.note(d.clone());
                    (d, Some(h), inst.status)
                }
                AgentStep::Check => {
                    if inst.status != S::Constructed {
                        return Err(invalid(inst));
                    }
                    let a = inst.candidate.clone().expect("constructed");
                    let checks = scenarios::run_checks(inst, &a);
                    let passed = checks.iter().filter(|c| c.passed).count();
                    let d = format!(
                        "{} of {} deterministic checks passed: {}",
                        passed,
                        checks.len(),
                        checks
                            .iter()
                            .map(|c| format!("{}={}", c.id, c.passed))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    inst.checks = checks;
                    inst.status = S::Checked;
                    inst.note(d.clone());
                    (d, None, inst.status)
                }
                AgentStep::Simulate => {
                    if inst.status != S::Checked {
                        return Err(invalid(inst));
                    }
                    let a = inst.candidate.clone().expect("constructed");
                    let (rb, econ) = scenarios::simulate(inst, &a);
                    let d = rb.narrative.clone();
                    inst.replay = Some(rb);
                    inst.economics = econ;
                    inst.status = S::Simulated;
                    inst.note(d.clone());
                    (d, None, inst.status)
                }
                AgentStep::Assemble => {
                    if inst.status != S::Simulated {
                        return Err(invalid(inst));
                    }
                    let a = inst.candidate.clone().expect("constructed");
                    let rb = inst.replay.clone().expect("simulated");
                    let checks = inst.checks.clone();
                    let econ = inst.economics.clone();
                    let pkg = scenarios::assemble(inst, &a, &checks, &rb, econ.as_ref());
                    let [c, s, as_, j] = pkg.counts();
                    let d = format!("Evidence package assembled: {} rows ({c} checked, {s} simulated, {as_} assumed, {j} judgment). Claims complete correctness: {}.", pkg.items.len(), pkg.claims_complete_correctness);
                    inst.package = Some(pkg);
                    inst.status = S::Assembled;
                    inst.note(d.clone());
                    (d, None, inst.status)
                }
                AgentStep::Commit => {
                    if inst.status != S::Assembled {
                        return Err(invalid(inst));
                    }
                    let h = inst.current_commitment().expect("candidate");
                    if let Some(p) = inst.package.as_mut() {
                        p.claimed_commitment = h;
                    }
                    if let Some(s) = inst.displayed_summary.as_mut() {
                        s.claimed_commitment = h;
                    }
                    inst.status = S::Committed;
                    let d = format!("Committed to H = {} (revision {}).", h.hex(), inst.revision);
                    inst.note(d.clone());
                    (d, Some(h), inst.status)
                }
                AgentStep::Submit => {
                    if inst.status != S::Committed {
                        return Err(invalid(inst));
                    }
                    inst.status = S::Submitted;
                    let h = inst.current_commitment();
                    let d = format!("Submitted {} revision {} for guardian review. The agent holds no key that changes protocol state.", inst.proposal_id, inst.revision);
                    inst.note(d.clone());
                    (d, h, inst.status)
                }
                AgentStep::Revise => {
                    if inst.status != S::ChangesRequested {
                        return Err(invalid(inst));
                    }
                    inst.revision += 1;
                    let mut old = std::mem::take(&mut inst.approvals);
                    inst.invalidated_approvals.append(&mut old);
                    inst.authorization = None;
                    inst.status = S::Diagnosed;
                    let d = format!("Revision {} started: previous approvals invalidated; a new artifact commitment and a new approval process are required.", inst.revision);
                    inst.note(d.clone());
                    (d, None, inst.status)
                }
            }
        };
        self.audit(Some(key), "agent", &format!("{step:?}"), detail, commitment);
        Ok(status)
    }

    /// Run the agent pipeline through submission.
    pub fn agent_run_to_submission(&mut self, key: &str) -> EngineResult<ProposalStatus> {
        let mut status = self.scenario(key)?.status;
        if status == ProposalStatus::ChangesRequested {
            status = self.agent_step(key, AgentStep::Revise)?;
        }
        for step in AgentStep::pipeline() {
            let needed = matches!(
                (status, step),
                (ProposalStatus::Idle, AgentStep::Observe)
                    | (ProposalStatus::Observed, AgentStep::Detect)
                    | (ProposalStatus::Detected, AgentStep::Diagnose)
                    | (ProposalStatus::Diagnosed, AgentStep::Construct)
                    | (ProposalStatus::Constructed, AgentStep::Check)
                    | (ProposalStatus::Checked, AgentStep::Simulate)
                    | (ProposalStatus::Simulated, AgentStep::Assemble)
                    | (ProposalStatus::Assembled, AgentStep::Commit)
                    | (ProposalStatus::Committed, AgentStep::Submit)
            );
            if needed {
                status = self.agent_step(key, step)?;
            }
        }
        Ok(status)
    }

    // ----------------------------------------------------------------- Stage B

    pub fn set_surface_mode(&mut self, key: &str, mode: SurfaceMode) -> EngineResult<()> {
        let before = self.scenario(key)?.current_commitment();
        self.scenario_mut(key)?.surface_mode = mode;
        let after = self.scenario(key)?.current_commitment();
        debug_assert_eq!(before, after);
        self.audit(
            Some(key),
            "guardian",
            "surface mode",
            format!("{:?}; proposal and H unchanged.", mode),
            after,
        );
        Ok(())
    }

    pub fn surface(&self, key: &str, mode: SurfaceMode) -> EngineResult<SurfaceView> {
        let inst = self.scenario(key)?;
        let pkg = inst
            .package
            .as_ref()
            .ok_or_else(|| EngineError::InvalidTransition {
                action: "view surface".into(),
                status: inst.status,
            })?;
        Ok(project(pkg, mode))
    }

    pub fn compare_surfaces(&self, key: &str) -> EngineResult<SurfaceComparison> {
        let inst = self.scenario(key)?;
        let pkg = inst
            .package
            .as_ref()
            .ok_or_else(|| EngineError::InvalidTransition {
                action: "compare surfaces".into(),
                status: inst.status,
            })?;
        Ok(compare(pkg))
    }

    pub fn guardian_inspect(&mut self, key: &str) -> EngineResult<ProposalStatus> {
        let status = {
            let inst = self.scenario_mut(key)?;
            match inst.status {
                ProposalStatus::Submitted => {
                    inst.status = ProposalStatus::UnderReview;
                    inst.note("Guardians opened the package.");
                }
                ProposalStatus::UnderReview
                | ProposalStatus::Approved
                | ProposalStatus::ChangesRequested
                | ProposalStatus::Rejected => {}
                _ => {
                    return Err(EngineError::InvalidTransition {
                        action: "inspect".into(),
                        status: inst.status,
                    })
                }
            }
            inst.status
        };
        self.audit(
            Some(key),
            "guardian",
            "inspect",
            "Opened the evidence package.",
            None,
        );
        Ok(status)
    }

    /// Recompute H from the artifact as held, independently of the agent's claim.
    pub fn guardian_recompute(&mut self, key: &str) -> EngineResult<RecomputeResult> {
        let result = {
            let inst = self.scenario_mut(key)?;
            let a = inst
                .candidate
                .as_ref()
                .ok_or_else(|| EngineError::InvalidTransition {
                    action: "recompute H".into(),
                    status: inst.status,
                })?;
            if inst.status == ProposalStatus::Submitted {
                inst.status = ProposalStatus::UnderReview;
            }
            let canon = a
                .manifest_canonical()
                .map_err(|e| EngineError::Other(e.to_string()))?;
            let computed = crate::canonical::compute_commitment(&canon, &a.payload);
            let claimed = inst
                .displayed_summary
                .as_ref()
                .map(|s| s.claimed_commitment)
                .unwrap_or(computed);
            RecomputeResult {
                computed,
                claimed,
                matches: computed == claimed,
                manifest_canonical_len: canon.len(),
                payload_len: a.payload.len(),
            }
        };
        self.audit(
            Some(key),
            "guardian",
            "recompute H",
            if result.matches {
                "Recomputed H matches the displayed value.".to_string()
            } else {
                format!(
                    "MISMATCH: recomputed {} vs displayed {}.",
                    result.computed.hex(),
                    result.claimed.hex()
                )
            },
            Some(result.computed),
        );
        Ok(result)
    }

    pub fn set_quorum_policy(
        &mut self,
        key: &str,
        policy_id: &str,
        custom: Option<(usize, usize)>,
    ) -> EngineResult<QuorumPolicy> {
        let policy = match custom {
            Some((m, n)) if m >= 1 && n >= m && n <= 9 => QuorumPolicy::custom(m, n),
            Some(_) => {
                return Err(EngineError::Other(
                    "custom quorum must satisfy 1 <= m <= n <= 9".into(),
                ))
            }
            None => quorum_presets()
                .into_iter()
                .find(|p| p.id == policy_id)
                .ok_or_else(|| EngineError::Other(format!("unknown quorum preset {policy_id}")))?,
        };
        let (note, h) = {
            let inst = self.scenario_mut(key)?;
            if matches!(
                inst.status,
                ProposalStatus::Executed | ProposalStatus::Escalated
            ) {
                return Err(EngineError::InvalidTransition {
                    action: "change quorum policy".into(),
                    status: inst.status,
                });
            }
            if inst.guardians.len() != policy.n {
                inst.guardians = GuardianSet::local_test(policy.n);
                let mut old = std::mem::take(&mut inst.approvals);
                inst.invalidated_approvals.append(&mut old);
                inst.authorization = None;
                if inst.status == ProposalStatus::Approved {
                    inst.status = ProposalStatus::UnderReview;
                }
            }
            inst.quorum_policy = policy.clone();
            // Re-evaluate with the approvals on file.
            let h = inst.current_commitment();
            let mut note = format!("Quorum policy set to {}.", policy.label);
            if let (Some(h), true) = (
                h,
                matches!(
                    inst.status,
                    ProposalStatus::UnderReview
                        | ProposalStatus::Submitted
                        | ProposalStatus::Approved
                ),
            ) {
                match evaluate_quorum(
                    &inst.guardians,
                    &policy,
                    &inst.approvals,
                    &inst.proposal_id,
                    inst.revision,
                    h,
                    0,
                ) {
                    Ok(auth) => {
                        inst.authorization = Some(auth);
                        inst.status = ProposalStatus::Approved;
                        note.push_str(" Quorum met with the approvals on file.");
                    }
                    Err(_) => {
                        inst.authorization = None;
                        if inst.status == ProposalStatus::Approved {
                            inst.status = ProposalStatus::UnderReview;
                        }
                        note.push_str(" Quorum not met with the approvals on file.");
                    }
                }
            }
            inst.note(note.clone());
            (note, h)
        };
        self.audit(Some(key), "reviewer", "quorum policy", note, h);
        Ok(policy)
    }

    pub fn guardian_decide(
        &mut self,
        key: &str,
        guardian_id: &str,
        decision: Decision,
        note: &str,
    ) -> EngineResult<GuardianOutcome> {
        let seq = self.seq + 1;
        let outcome = {
            let inst = self.scenario_mut(key)?;
            if !matches!(
                inst.status,
                ProposalStatus::Submitted | ProposalStatus::UnderReview
            ) {
                return Err(EngineError::InvalidTransition {
                    action: format!("{decision:?}"),
                    status: inst.status,
                });
            }
            let a = inst
                .candidate
                .as_ref()
                .ok_or_else(|| EngineError::InvalidTransition {
                    action: "sign".into(),
                    status: inst.status,
                })?;
            // The guardian signs the H they recompute, never the displayed value.
            let h = a
                .commitment()
                .map_err(|e| EngineError::Other(e.to_string()))?;
            let approval = sign_approval(
                &inst.guardians,
                guardian_id,
                &inst.proposal_id,
                inst.revision,
                decision,
                h,
                note,
                inst.surface_mode,
            )?;
            inst.approvals.retain(|x| x.guardian_id != guardian_id);
            inst.approvals.push(approval.clone());
            inst.status = ProposalStatus::UnderReview;
            let (status, authorization, quorum_note) = match decision {
                Decision::Reject => {
                    inst.status = ProposalStatus::Rejected;
                    inst.authorization = None;
                    (inst.status, None, format!("{guardian_id} rejected H."))
                }
                Decision::RequestChanges => {
                    inst.status = ProposalStatus::ChangesRequested;
                    inst.authorization = None;
                    (inst.status, None, format!("{guardian_id} requested changes. A changed proposal needs a new H and a new approval."))
                }
                Decision::Approve => match evaluate_quorum(
                    &inst.guardians,
                    &inst.quorum_policy,
                    &inst.approvals,
                    &inst.proposal_id,
                    inst.revision,
                    h,
                    seq,
                ) {
                    Ok(auth) => {
                        inst.status = ProposalStatus::Approved;
                        inst.authorization = Some(auth.clone());
                        (
                            inst.status,
                            Some(auth),
                            format!(
                                "Quorum met: {} of {} signatures over exactly {}.",
                                inst.quorum_policy.m,
                                inst.quorum_policy.n,
                                h.short()
                            ),
                        )
                    }
                    Err(QuorumError::NotMet { have, need }) => (
                        inst.status,
                        None,
                        format!("{have} of {need} signatures over {} so far.", h.short()),
                    ),
                    Err(e) => return Err(e.into()),
                },
            };
            inst.note(quorum_note.clone());
            GuardianOutcome {
                approval,
                status,
                authorization,
                quorum_note,
            }
        };
        self.audit(
            Some(key),
            guardian_id,
            &format!("{decision:?}"),
            format!(
                "{} (viewed surface {:?})",
                outcome.quorum_note, outcome.approval.surface_mode_viewed
            ),
            Some(outcome.approval.commitment),
        );
        Ok(outcome)
    }

    // ----------------------------------------------------------------- Stage C

    /// Substitute the artifact after review. The displayed summary is left as it was.
    pub fn tamper(&mut self, key: &str, variant: TamperVariant) -> EngineResult<TamperRecord> {
        let rec = {
            let inst = self.scenario_mut(key)?;
            if !matches!(
                inst.status,
                ProposalStatus::Committed
                    | ProposalStatus::Submitted
                    | ProposalStatus::UnderReview
                    | ProposalStatus::Approved
                    | ProposalStatus::ExecutionRefused
            ) {
                return Err(EngineError::InvalidTransition {
                    action: "tamper".into(),
                    status: inst.status,
                });
            }
            let original = inst
                .original_candidate
                .clone()
                .or_else(|| inst.candidate.clone())
                .expect("candidate exists");
            let (tampered, description) = scenarios::tamper(&original, variant);
            let rec = TamperRecord {
                variant,
                description,
                after_status: inst.status,
                original_commitment: original.commitment().expect("canonical"),
                tampered_commitment: tampered.commitment().expect("canonical"),
            };
            inst.original_candidate = Some(original);
            inst.candidate = Some(tampered);
            inst.tamper = Some(rec.clone());
            inst.note(format!("[attack] {}", rec.description));
            rec
        };
        self.audit(
            Some(key),
            "attacker",
            "substitute artifact",
            rec.description.clone(),
            Some(rec.tampered_commitment),
        );
        Ok(rec)
    }

    pub fn restore_candidate(&mut self, key: &str) -> EngineResult<()> {
        let h = {
            let inst = self.scenario_mut(key)?;
            let Some(orig) = inst.original_candidate.take() else {
                return Ok(());
            };
            inst.candidate = Some(orig);
            inst.tamper = None;
            if inst.status == ProposalStatus::ExecutionRefused {
                inst.status = if inst.authorization.is_some() {
                    ProposalStatus::Approved
                } else {
                    ProposalStatus::UnderReview
                };
                inst.execution = None;
            }
            inst.note("Original artifact restored.");
            inst.current_commitment()
        };
        self.audit(
            Some(key),
            "reviewer",
            "restore artifact",
            "Original artifact restored.",
            h,
        );
        Ok(())
    }

    pub fn execute(&mut self, key: &str) -> EngineResult<ExecutionOutcome> {
        let (outcome, h) = {
            let inst = self.scenario_mut(key)?;
            if !matches!(
                inst.status,
                ProposalStatus::Approved | ProposalStatus::ExecutionRefused
            ) {
                return Err(EngineError::InvalidTransition {
                    action: "execute".into(),
                    status: inst.status,
                });
            }
            let auth =
                inst.authorization
                    .clone()
                    .ok_or_else(|| EngineError::InvalidTransition {
                        action: "execute without authorization".into(),
                        status: inst.status,
                    })?;
            let artifact = inst.candidate.clone().expect("candidate");
            let proposal_id = inst.proposal_id.clone();
            let mut state = inst.state.clone();
            let outcome = match execute(&auth, &inst.guardians, &artifact, &proposal_id, &mut state)
            {
                Ok(report) => {
                    inst.state = state;
                    inst.status = if report.post_state.ok {
                        ProposalStatus::Executed
                    } else {
                        ProposalStatus::Escalated
                    };
                    inst.note(report.statement.clone());
                    ExecutionOutcome::Applied { report }
                }
                Err(refusal) => {
                    inst.status = ProposalStatus::ExecutionRefused;
                    inst.note(refusal.summary());
                    ExecutionOutcome::Refused { refusal }
                }
            };
            inst.execution = Some(outcome.clone());
            (outcome, auth.commitment)
        };
        let detail = match &outcome {
            ExecutionOutcome::Applied { report } => report.statement.clone(),
            ExecutionOutcome::Refused { refusal } => refusal.summary(),
        };
        self.audit(Some(key), "executor", "execute", detail, Some(h));
        Ok(outcome)
    }

    /// Run the verifier in-process over the artifact as held. The CLI does the
    /// same out of process on exported files.
    pub fn verify_independently(&self, key: &str) -> EngineResult<VerifierReport> {
        let inst = self.scenario(key)?;
        let a = inst
            .candidate
            .as_ref()
            .ok_or_else(|| EngineError::InvalidTransition {
                action: "verify".into(),
                status: inst.status,
            })?;
        let manifest_json = serde_json::to_vec_pretty(&a.manifest)
            .map_err(|e| EngineError::Other(e.to_string()))?;
        let expected = inst
            .authorization
            .as_ref()
            .map(|x| x.commitment)
            .or_else(|| {
                inst.displayed_summary
                    .as_ref()
                    .map(|s| s.claimed_commitment)
            });
        Ok(verify(
            &manifest_json,
            &a.payload,
            expected,
            inst.authorization.as_ref(),
            Some(&inst.guardians.public()),
        ))
    }

    /// Files for out-of-process verification with `gup-verify`.
    pub fn verification_bundle(&self, key: &str) -> EngineResult<Vec<(String, Vec<u8>)>> {
        let inst = self.scenario(key)?;
        let a = inst
            .candidate
            .as_ref()
            .ok_or_else(|| EngineError::InvalidTransition {
                action: "export bundle".into(),
                status: inst.status,
            })?;
        let mut files = vec![
            (
                "manifest.json".to_string(),
                serde_json::to_vec_pretty(&a.manifest)
                    .map_err(|e| EngineError::Other(e.to_string()))?,
            ),
            ("payload.bin".to_string(), a.payload.clone()),
            (
                "guardians.json".to_string(),
                serde_json::to_vec_pretty(&inst.guardians.public())
                    .map_err(|e| EngineError::Other(e.to_string()))?,
            ),
        ];
        if let Some(s) = &inst.displayed_summary {
            files.push((
                "displayed_summary.txt".into(),
                format!(
                    "{}\n{}\n{}\nclaimed H: {}\n(rendered by {}; NOT an input to the verifier)\n",
                    s.title,
                    s.summary,
                    s.lines.join("\n"),
                    s.claimed_commitment.hex(),
                    s.rendered_by
                )
                .into_bytes(),
            ));
        }
        if let Some(auth) = &inst.authorization {
            files.push((
                "authorization.json".into(),
                serde_json::to_vec_pretty(auth).map_err(|e| EngineError::Other(e.to_string()))?,
            ));
            files.push((
                "expected_commitment.txt".into(),
                format!("{}\n", auth.commitment.hex()).into_bytes(),
            ));
        } else if let Some(s) = &inst.displayed_summary {
            files.push((
                "expected_commitment.txt".into(),
                format!("{}\n", s.claimed_commitment.hex()).into_bytes(),
            ));
        }
        files.push((
            "README.txt".into(),
            format!(
                "Guarded Upgrade Path verification bundle for proposal {} revision {}\n\nRecompute H from the tagged inputs only:\n  gup-verify hash --manifest manifest.json --payload payload.bin\n\nCompare with the approved value:\n  gup-verify verify --manifest manifest.json --payload payload.bin --expected $(cat expected_commitment.txt)\n\nVerify the guardian authorization (signatures over H, quorum):\n  gup-verify authorization --manifest manifest.json --payload payload.bin --authorization authorization.json --guardians guardians.json\n\nReproduce without gup-verify:\n  gup-verify preimage --manifest manifest.json --payload payload.bin --out preimage.bin && sha256sum preimage.bin\n\ndisplayed_summary.txt is what the agent's interface showed. It is not an input to any verification step.\n",
                inst.proposal_id, inst.revision
            )
            .into_bytes(),
        ));
        Ok(files)
    }

    // ---------------------------------------------------------------- Recovery

    pub fn assess_recovery(&mut self, key: &str) -> EngineResult<RecoveryAssessment> {
        let a = {
            let inst = self.scenario_mut(key)?;
            let a = assess(&inst.state);
            inst.recovery = Some(a.clone());
            a
        };
        self.audit(
            Some(key),
            "reviewer",
            "recovery assessment",
            a.statement.clone(),
            None,
        );
        Ok(a)
    }

    /// After execution, let users act under the new version for `periods` periods.
    pub fn simulate_users_act(&mut self, key: &str, periods: u64) -> EngineResult<String> {
        let detail = {
            let inst = self.scenario_mut(key)?;
            if !matches!(
                inst.status,
                ProposalStatus::Executed | ProposalStatus::Escalated
            ) {
                return Err(EngineError::InvalidTransition {
                    action: "users act".into(),
                    status: inst.status,
                });
            }
            let periods = periods.clamp(1, 6);
            let start =
                (inst.state.block / inst.state.period_length + 1) * inst.state.period_length;
            let stream = shift_stream(
                build_stream(
                    0xACED ^ inst.state.block,
                    periods,
                    1_200 * periods as usize,
                    &[],
                    0,
                    false,
                ),
                start,
            );
            let before_paid = inst.state.distribution.payout_count;
            let (claims, paid, _) = run_stream(&mut inst.state, &stream);
            let gross: i64 = inst
                .state
                .distribution
                .per_period
                .values()
                .filter(|p| p.period >= start / inst.state.period_length)
                .map(|p| p.gross)
                .sum();
            inst.state.record_effect(
                EffectKind::UserAction,
                format!(
                    "{paid} claims paid under {} over {periods} periods",
                    inst.state.distribution_package().version
                ),
                Some(gross),
                paid,
            );
            if inst.state.distribution.format_version == 2 {
                inst.state.record_effect(
                    EffectKind::Integration,
                    "Indexer switched to reading packed ClaimRecords",
                    None,
                    1,
                );
            }
            let d = format!("{claims} claims processed, {paid} paid ({} units) under {} in blocks {}-{}. Payout count {} → {}.", gross, inst.state.distribution_package().version, stream.first_block, stream.last_block, before_paid, inst.state.distribution.payout_count);
            inst.note(d.clone());
            d
        };
        self.audit(
            Some(key),
            "users",
            "act under new version",
            detail.clone(),
            None,
        );
        Ok(detail)
    }

    pub fn attempt_code_revert(&mut self, key: &str) -> EngineResult<RevertAttempt> {
        let attempt = {
            let inst = self.scenario_mut(key)?;
            if inst.state.history.is_empty() {
                return Err(EngineError::Recovery(RecoveryError::NothingApplied));
            }
            let attempt = match attempt_code_revert(&inst.state) {
                Ok((assessment, draft)) => RevertAttempt { blocked: false, reason: "Code revert can be drafted as a new proposal; it is not a recovery of anything that already happened.".into(), assessment, draft: Some(draft) },
                Err(RecoveryError::Blocked(reason)) => RevertAttempt { blocked: true, reason, assessment: assess(&inst.state), draft: None },
                Err(e) => return Err(e.into()),
            };
            inst.recovery = Some(attempt.assessment.clone());
            inst.revert_attempt = Some(attempt.clone());
            inst.note(if attempt.blocked {
                format!("Automatic code revert BLOCKED: {}", attempt.reason)
            } else {
                attempt.reason.clone()
            });
            attempt
        };
        self.audit(
            Some(key),
            "executor",
            "attempt code revert",
            if attempt.blocked {
                format!("Blocked: {}", attempt.reason)
            } else {
                attempt.reason.clone()
            },
            None,
        );
        Ok(attempt)
    }

    /// Open a distinct recovery proposal (new proposal id, new H, normal approval).
    pub fn draft_recovery(&mut self, key: &str) -> EngineResult<Artifact> {
        let (artifact, pid) = {
            let inst = self.scenario_mut(key)?;
            let artifact = draft_recovery_proposal(&inst.state)?;
            inst.completed.push(CompletedProposal {
                proposal_id: inst.proposal_id.clone(),
                commitment: inst.authorization.as_ref().map(|a| a.commitment),
                status: inst.status,
                summary: inst
                    .candidate
                    .as_ref()
                    .map(|c| c.manifest.summary.clone())
                    .unwrap_or_default(),
            });
            inst.proposal_counter += 1;
            inst.proposal_id = format!("{}-{}", inst.meta.key, inst.proposal_counter);
            inst.revision = 1;
            inst.status = ProposalStatus::Constructed;
            let h = artifact.commitment().expect("canonical");
            inst.displayed_summary = Some(DisplayedSummary {
                title: format!("Recovery proposal {}", inst.proposal_id),
                summary: artifact.manifest.summary.clone(),
                lines: vec![
                    "Distinct proposal with its own H and its own approval".into(),
                    "Nothing that already happened is undone".into(),
                ],
                claimed_commitment: h,
                rendered_by: "proposal agent (simulated UI)".into(),
            });
            inst.candidate_diff = Some(
                artifact
                    .manifest
                    .migration
                    .as_ref()
                    .map(|m| m.description.clone())
                    .unwrap_or_else(|| "Reinstall prior implementation".into()),
            );
            inst.candidate = Some(artifact.clone());
            inst.original_candidate = None;
            inst.checks.clear();
            inst.replay = None;
            inst.economics = None;
            inst.package = None;
            inst.approvals.clear();
            inst.authorization = None;
            inst.execution = None;
            inst.tamper = None;
            inst.note(format!(
                "Recovery proposal {} drafted: {}",
                inst.proposal_id, artifact.manifest.summary
            ));
            (artifact, inst.proposal_id.clone())
        };
        self.audit(
            Some(key),
            "agent",
            "draft recovery proposal",
            format!("{pid}: {}", artifact.manifest.summary),
            artifact.commitment().ok(),
        );
        Ok(artifact)
    }

    // ---------------------------------------------------------------- Incident

    fn incident_mut(&mut self, key: &str) -> EngineResult<&mut IncidentState> {
        let inst = self.scenario_mut(key)?;
        inst.incident
            .as_mut()
            .ok_or_else(|| EngineError::Other("this scenario has no incident".into()))
    }

    pub fn incident_set_policy(
        &mut self,
        key: &str,
        policy_id: &str,
    ) -> EngineResult<IncidentPolicy> {
        let policy = incident_policies()
            .into_iter()
            .find(|p| p.id == policy_id)
            .ok_or_else(|| EngineError::Other(format!("unknown incident policy {policy_id}")))?;
        let inc = self.incident_mut(key)?;
        inc.policy_id = policy.id.clone();
        inc.outcomes = compare_policies(&inc.params);
        inc.log.push(format!("Policy selected for comparison: {} (hypothetical; normal approval remains the default).", policy.label));
        self.audit(
            Some(key),
            "reviewer",
            "incident policy",
            policy.label.clone(),
            None,
        );
        Ok(policy)
    }

    pub fn incident_set_params(
        &mut self,
        key: &str,
        params: IncidentParams,
    ) -> EngineResult<Vec<IncidentOutcome>> {
        let outcomes = {
            let inc = self.incident_mut(key)?;
            inc.params = params;
            inc.outcomes = compare_policies(&inc.params);
            inc.log.push(format!(
                "Parameters updated: quorum at block {}, loss {} per block, false positive {}.",
                inc.params.quorum_arrival_block,
                inc.params.exploit_loss_per_block,
                inc.params.false_positive
            ));
            inc.outcomes.clone()
        };
        self.audit(
            Some(key),
            "reviewer",
            "incident parameters",
            format!("{} policy outcomes recomputed.", outcomes.len()),
            None,
        );
        Ok(outcomes)
    }

    /// The agent asks to act on a rung under the selected policy.
    pub fn incident_agent_request(
        &mut self,
        key: &str,
        rung: Rung,
    ) -> EngineResult<EmergencyGrant> {
        let result = {
            let inst = self.scenario_mut(key)?;
            let block = inst.state.block;
            let inc = inst
                .incident
                .as_mut()
                .ok_or_else(|| EngineError::Other("no incident".into()))?;
            let policy = incident_policies()
                .into_iter()
                .find(|p| p.id == inc.policy_id)
                .expect("policy exists");
            match agent_request_action(&policy, &inc.incident_id, rung, block, inc.open) {
                Ok(grant) => {
                    match rung {
                        Rung::LimitOneFunction => {
                            inst.state
                                .pause
                                .functions
                                .insert("claimFor(address[])".into());
                        }
                        Rung::PauseOnePackage => {
                            inst.state.pause.packages.insert(DISTRIBUTION.into());
                        }
                        Rung::PauseProtocol => {
                            inst.state.pause.protocol = true;
                        }
                        _ => {}
                    }
                    if rung.changes_protocol_state() {
                        inst.state.record_effect(
                            EffectKind::Pause,
                            format!("Agent used pre-authorized action: {}", rung.label()),
                            None,
                            1,
                        );
                    }
                    let inc = inst.incident.as_mut().expect("incident");
                    inc.grant = Some(grant.clone());
                    inc.log.push(format!(
                        "Agent acted on rung '{}' at block {} under policy {} (expires {:?}).",
                        rung.label(),
                        block,
                        policy.label,
                        grant.expires_at_block
                    ));
                    Ok(grant)
                }
                Err(e) => {
                    let inc = inst.incident.as_mut().expect("incident");
                    inc.log
                        .push(format!("Agent request for '{}' REFUSED: {e}", rung.label()));
                    Err(e)
                }
            }
        };
        match &result {
            Ok(g) => self.audit(
                Some(key),
                "agent",
                "emergency action",
                format!(
                    "{} granted under {} (expires {:?})",
                    g.rung.label(),
                    g.policy_id,
                    g.expires_at_block
                ),
                None,
            ),
            Err(e) => self.audit(
                Some(key),
                "agent",
                "emergency action refused",
                e.to_string(),
                None,
            ),
        }
        Ok(result?)
    }

    pub fn incident_agent_lift(&mut self, key: &str) -> EngineResult<()> {
        let err = {
            let inc = self.incident_mut(key)?;
            let r = match &inc.grant {
                Some(g) => agent_attempt_lift(g).err(),
                None => Some(AuthorityRefusal::AgentCannotLift),
            };
            if let Some(e) = &r {
                inc.log
                    .push(format!("Agent attempt to lift containment REFUSED: {e}"));
            }
            r
        };
        self.audit(
            Some(key),
            "agent",
            "lift containment refused",
            err.as_ref().map(|e| e.to_string()).unwrap_or_default(),
            None,
        );
        match err {
            Some(e) => Err(e.into()),
            None => Ok(()),
        }
    }

    pub fn incident_guardians_lift(&mut self, key: &str) -> EngineResult<()> {
        {
            let inst = self.scenario_mut(key)?;
            inst.state.pause = Default::default();
            inst.state.record_effect(
                EffectKind::Pause,
                "Guardians lifted the containment",
                None,
                1,
            );
            let inc = inst
                .incident
                .as_mut()
                .ok_or_else(|| EngineError::Other("no incident".into()))?;
            if let Some(g) = inc.grant.as_mut() {
                g.lifted_by_guardians = true;
            }
            inc.log
                .push("Guardians lifted the containment (guardian decision).".into());
        }
        self.audit(
            Some(key),
            "guardians",
            "lift containment",
            "Pause state cleared by guardian decision.",
            None,
        );
        Ok(())
    }

    pub fn incident_check_grant(&self, key: &str) -> EngineResult<Result<(), AuthorityRefusal>> {
        let inst = self.scenario(key)?;
        let inc = inst
            .incident
            .as_ref()
            .ok_or_else(|| EngineError::Other("no incident".into()))?;
        Ok(match &inc.grant {
            Some(_) if !inc.open => Err(AuthorityRefusal::IncidentClosed),
            Some(g) => check_grant(g, &inc.incident_id, inst.state.block),
            None => Err(AuthorityRefusal::NotPreAuthorized(inc.policy_id.clone())),
        })
    }

    pub fn incident_close(&mut self, key: &str) -> EngineResult<()> {
        {
            let inc = self.incident_mut(key)?;
            inc.open = false;
            inc.log
                .push("Incident closed by guardians. Any standing grant is void.".into());
        }
        self.audit(
            Some(key),
            "guardians",
            "close incident",
            "No standing authority remains with the agent.",
            None,
        );
        Ok(())
    }

    pub fn advance_blocks(&mut self, key: &str, blocks: u64) -> EngineResult<u64> {
        let b = {
            let inst = self.scenario_mut(key)?;
            inst.state.block += blocks.min(10_000);
            inst.state.block
        };
        self.audit(Some(key), "clock", "advance", format!("block {b}"), None);
        Ok(b)
    }

    // ------------------------------------------------------------------- Notes

    pub fn add_note(&mut self, note: ReviewNote) -> u64 {
        self.seq += 1;
        let mut n = note;
        n.id = self.seq;
        n.created_seq = self.seq;
        self.notes.push(n);
        self.audit(
            None,
            "reviewer",
            "note added",
            "Local review note saved.",
            None,
        );
        self.seq
    }

    pub fn update_note(&mut self, note: ReviewNote) -> bool {
        if let Some(existing) = self.notes.iter_mut().find(|n| n.id == note.id) {
            let id = existing.id;
            let created = existing.created_seq;
            *existing = note;
            existing.id = id;
            existing.created_seq = created;
            self.audit(None, "reviewer", "note updated", format!("note {id}"), None);
            true
        } else {
            false
        }
    }

    pub fn delete_note(&mut self, id: u64) -> bool {
        let before = self.notes.len();
        self.notes.retain(|n| n.id != id);
        let removed = self.notes.len() != before;
        if removed {
            self.audit(None, "reviewer", "note deleted", format!("note {id}"), None);
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approved_a() -> Workspace {
        let mut ws = Workspace::new();
        ws.agent_run_to_submission("A").unwrap();
        ws.guardian_decide("A", "guardian-1", Decision::Approve, "")
            .unwrap();
        ws.guardian_decide("A", "guardian-2", Decision::Approve, "")
            .unwrap();
        assert_eq!(ws.scenario("A").unwrap().status, ProposalStatus::Approved);
        ws
    }

    #[test]
    fn agent_cannot_skip_steps_or_authorize() {
        let mut ws = Workspace::new();
        assert!(matches!(
            ws.agent_step("A", AgentStep::Commit),
            Err(EngineError::InvalidTransition { .. })
        ));
        ws.agent_run_to_submission("A").unwrap();
        // No authorization exists; execution is refused by the state machine.
        assert!(matches!(
            ws.execute("A"),
            Err(EngineError::InvalidTransition { .. })
        ));
        assert!(ws.scenario("A").unwrap().authorization.is_none());
    }

    #[test]
    fn full_happy_path_a() {
        let mut ws = approved_a();
        let out = ws.execute("A").unwrap();
        assert!(matches!(out, ExecutionOutcome::Applied { .. }));
        let inst = ws.scenario("A").unwrap();
        assert_eq!(inst.status, ProposalStatus::Executed);
        assert_eq!(inst.state.distribution_package().version, "2.3.1");
        assert_eq!(inst.state.history.len(), 1);
    }

    #[test]
    fn surfaces_never_change_h() {
        let mut ws = Workspace::new();
        ws.agent_run_to_submission("A").unwrap();
        let h = ws.scenario("A").unwrap().current_commitment().unwrap();
        for m in SurfaceMode::all() {
            ws.set_surface_mode("A", m).unwrap();
            let v = ws.surface("A", m).unwrap();
            assert_eq!(v.commitment, h);
        }
        let cmp = ws.compare_surfaces("A").unwrap();
        assert!(cmp.commitment_identical && cmp.package_unchanged);
        assert!(!cmp.views[0].omitted.is_empty());
    }

    #[test]
    fn changes_requested_invalidates_approvals_and_changes_h() {
        let mut ws = Workspace::new();
        ws.agent_run_to_submission("A").unwrap();
        let h1 = ws.scenario("A").unwrap().current_commitment().unwrap();
        ws.guardian_decide("A", "guardian-1", Decision::Approve, "")
            .unwrap();
        ws.guardian_decide(
            "A",
            "guardian-2",
            Decision::RequestChanges,
            "re-verify the scheduler",
        )
        .unwrap();
        assert_eq!(
            ws.scenario("A").unwrap().status,
            ProposalStatus::ChangesRequested
        );
        ws.agent_run_to_submission("A").unwrap();
        let inst = ws.scenario("A").unwrap();
        assert_eq!(inst.revision, 2);
        assert!(inst.approvals.is_empty());
        assert_eq!(inst.invalidated_approvals.len(), 2);
        assert_ne!(inst.current_commitment().unwrap(), h1);
    }

    #[test]
    fn substitution_after_approval_is_refused() {
        let mut ws = approved_a();
        let rec = ws.tamper("A", TamperVariant::DivertFee).unwrap();
        assert_ne!(rec.original_commitment, rec.tampered_commitment);
        // The displayed summary still shows the approved value.
        assert_eq!(
            ws.scenario("A")
                .unwrap()
                .displayed_summary
                .as_ref()
                .unwrap()
                .claimed_commitment,
            rec.original_commitment
        );
        let out = ws.execute("A").unwrap();
        match out {
            ExecutionOutcome::Refused {
                refusal:
                    ExecutionRefusal::CommitmentMismatch {
                        expected, computed, ..
                    },
            } => {
                assert_eq!(expected, rec.original_commitment);
                assert_eq!(computed, rec.tampered_commitment);
            }
            other => panic!("expected refusal, got {other:?}"),
        }
        assert_eq!(
            ws.scenario("A")
                .unwrap()
                .state
                .distribution_package()
                .version,
            "2.3.0"
        );
        let rep = ws.verify_independently("A").unwrap();
        assert_eq!(rep.verdict, crate::verifier::Verdict::Reject);
        ws.restore_candidate("A").unwrap();
        assert!(matches!(
            ws.execute("A").unwrap(),
            ExecutionOutcome::Applied { .. }
        ));
    }

    #[test]
    fn guardian_recompute_detects_pre_signing_substitution() {
        let mut ws = Workspace::new();
        ws.agent_run_to_submission("A").unwrap();
        ws.tamper("A", TamperVariant::ManifestAndPayload).unwrap();
        let r = ws.guardian_recompute("A").unwrap();
        assert!(!r.matches);
    }

    #[test]
    fn quorum_policy_is_configurable() {
        let mut ws = Workspace::new();
        ws.agent_run_to_submission("B").unwrap();
        ws.set_quorum_policy("B", "lighter-1-of-3", None).unwrap();
        ws.guardian_decide("B", "guardian-3", Decision::Approve, "")
            .unwrap();
        assert_eq!(ws.scenario("B").unwrap().status, ProposalStatus::Approved);
        ws.set_quorum_policy("B", "unanimous-3-of-3", None).unwrap();
        assert_eq!(
            ws.scenario("B").unwrap().status,
            ProposalStatus::UnderReview
        );
        ws.set_quorum_policy("B", "", Some((3, 5))).unwrap();
        assert_eq!(ws.scenario("B").unwrap().guardians.len(), 5);
    }

    #[test]
    fn migration_then_revert_is_blocked() {
        let mut ws = Workspace::new();
        ws.agent_run_to_submission("C").unwrap();
        ws.guardian_decide("C", "guardian-1", Decision::Approve, "")
            .unwrap();
        ws.guardian_decide("C", "guardian-2", Decision::Approve, "")
            .unwrap();
        assert!(matches!(
            ws.execute("C").unwrap(),
            ExecutionOutcome::Applied { .. }
        ));
        assert_eq!(
            ws.scenario("C").unwrap().state.distribution.format_version,
            2
        );
        ws.simulate_users_act("C", 2).unwrap();
        let attempt = ws.attempt_code_revert("C").unwrap();
        assert!(attempt.blocked);
        assert!(attempt.assessment.misreads_if_reverted > 0);
        let draft = ws.draft_recovery("C").unwrap();
        assert_eq!(
            draft.manifest.change_kind,
            crate::artifact::ChangeKind::Recovery
        );
        assert_eq!(ws.scenario("C").unwrap().proposal_id, "C-2");
        // The recovery proposal goes through the normal path.
        ws.agent_step("C", AgentStep::Check).unwrap();
        ws.agent_step("C", AgentStep::Simulate).unwrap();
        ws.agent_step("C", AgentStep::Assemble).unwrap();
        ws.agent_step("C", AgentStep::Commit).unwrap();
        ws.agent_step("C", AgentStep::Submit).unwrap();
        ws.guardian_decide("C", "guardian-1", Decision::Approve, "")
            .unwrap();
        ws.guardian_decide("C", "guardian-3", Decision::Approve, "")
            .unwrap();
        assert!(matches!(
            ws.execute("C").unwrap(),
            ExecutionOutcome::Applied { .. }
        ));
        assert_eq!(
            ws.scenario("C").unwrap().state.distribution.format_version,
            1
        );
    }

    #[test]
    fn incident_authority_is_bounded() {
        let mut ws = Workspace::new();
        assert!(ws
            .incident_agent_request("D", Rung::LimitOneFunction)
            .is_err());
        ws.incident_set_policy("D", "pre-limit-function").unwrap();
        assert!(ws.incident_agent_request("D", Rung::PauseProtocol).is_err());
        let g = ws
            .incident_agent_request("D", Rung::LimitOneFunction)
            .unwrap();
        assert!(ws
            .scenario("D")
            .unwrap()
            .state
            .pause
            .functions
            .contains("claimFor(address[])"));
        assert!(ws.incident_agent_lift("D").is_err());
        ws.advance_blocks("D", 500).unwrap();
        assert!(ws.incident_check_grant("D").unwrap().is_err());
        ws.incident_guardians_lift("D").unwrap();
        assert!(!ws.scenario("D").unwrap().state.pause.any());
        ws.incident_close("D").unwrap();
        assert!(ws.incident_agent_request("D", Rung::Flag).is_err());
        assert_eq!(g.rung, Rung::LimitOneFunction);
    }

    #[test]
    fn every_case_passes_its_checks_and_executes_with_a_clean_post_state() {
        for key in ["A", "B", "C", "D"] {
            let mut ws = Workspace::new();
            ws.agent_run_to_submission(key).unwrap();
            let pkg = ws.scenario(key).unwrap().package.clone().unwrap();
            assert!(
                pkg.technical_verdict.all_checked_passed,
                "{key}: {:?}",
                pkg.technical_verdict.checked_failed
            );
            ws.guardian_decide(key, "guardian-1", Decision::Approve, "")
                .unwrap();
            ws.guardian_decide(key, "guardian-2", Decision::Approve, "")
                .unwrap();
            match ws.execute(key).unwrap() {
                ExecutionOutcome::Applied { report } => {
                    assert!(
                        report.post_state.ok,
                        "{key}: {:?}",
                        report.post_state.escalation
                    );
                    assert_eq!(ws.scenario(key).unwrap().status, ProposalStatus::Executed);
                }
                other => panic!("{key}: expected applied, got {other:?}"),
            }
        }
    }

    #[test]
    fn workspace_roundtrips_through_json() {
        let mut ws = approved_a();
        ws.execute("A").unwrap();
        let json = ws.to_json().unwrap();
        let back = Workspace::from_json(&json).unwrap();
        assert_eq!(back, ws);
        assert_eq!(
            back.scenario("A").unwrap().stream.claims(),
            crate::simulation::FIXTURE_CLAIMS
        );
    }
}
