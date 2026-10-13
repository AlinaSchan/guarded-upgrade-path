// Type mirrors of the Rust view structs (serde JSON). The frontend renders
// these; it never computes authority decisions.

export type ProposalStatus =
  | "idle"
  | "observed"
  | "detected"
  | "diagnosed"
  | "constructed"
  | "checked"
  | "simulated"
  | "assembled"
  | "committed"
  | "submitted"
  | "under_review"
  | "changes_requested"
  | "rejected"
  | "approved"
  | "execution_refused"
  | "executed"
  | "escalated";

export type AgentStep = "observe" | "detect" | "diagnose" | "construct" | "check" | "simulate" | "assemble" | "commit" | "submit" | "revise";
export type Classification = "checked" | "simulated" | "assumed" | "judgment";
export type SurfaceMode = "minimal" | "structured" | "full";
export type Decision = "approve" | "reject" | "request_changes";
export type Rung = "flag" | "draft_fix" | "limit_one_function" | "pause_one_package" | "pause_protocol";
export type ChangeKind = "implementation_only" | "parameter_only" | "state_migration" | "incident_fix" | "recovery";
export type LayerStatus = "possible" | "blocked" | "partial" | "nothing";
export type Verdict = "accept" | "reject";
export type TamperVariant = "divert_fee" | "reroute_selector" | "silent_parameter" | "manifest_and_payload";
export type CandidateVariant = "default" | "flat_amount";
export type Concern = "technical_validity" | "desirability" | "both";
export type EffectKind = "transfer" | "permission_used" | "integration" | "user_action" | "pause";

export interface Tag {
  label: string;
  value: string;
}

export interface ScenarioMeta {
  id: string;
  key: string;
  title: string;
  tags: Tag[];
  technical_change: string;
  governance_effect: string;
  open_question: string;
  package_title: string;
  problem: string;
  change_kind: ChangeKind;
}

export interface Signal {
  name: string;
  value: string;
  note: string;
}

export interface Observation {
  block: number;
  period: number;
  package_version: string;
  signals: Signal[];
}

export interface Detection {
  title: string;
  query: string;
  block_range: [number, number];
  result: string;
  affected_wallets: number[];
  rows: string[];
}

export interface DisplayedSummary {
  title: string;
  summary: string;
  lines: string[];
  claimed_commitment: string;
  rendered_by: string;
}

export interface FunctionEntry {
  selector: string;
  name: string;
  impl_id: string;
  privileged: boolean;
}

export interface StorageSlot {
  index: number;
  name: string;
  type: string;
}

export interface StorageLayout {
  namespace: string;
  format_version: number;
  slots: StorageSlot[];
}

export interface RoleSet {
  upgrade_admin: string[];
  guardians: string[];
}

export interface MigrationSpec {
  id: string;
  description: string;
  from_format: number;
  to_format: number;
  proposer_reversibility_claim: string;
}

export interface ArtifactManifest {
  schema: string;
  package: string;
  from_version: string;
  to_version: string;
  change_kind: ChangeKind;
  summary: string;
  functions: FunctionEntry[];
  storage: StorageLayout;
  parameters: Record<string, number>;
  roles: RoleSet;
  migration: MigrationSpec | null;
  payload_sha256: string;
  produced_by: string;
}

export interface Artifact {
  manifest: ArtifactManifest;
  payload: string;
}

export interface ArtifactView {
  manifest: ArtifactManifest;
  manifest_json: string;
  payload_text: string;
  payload_len: number;
  payload_sha256: string;
  commitment: string;
  commitment_short: string;
}

export interface CheckResult {
  id: string;
  name: string;
  passed: boolean;
  classification: Classification;
  method: string;
  scope: string;
  output: string;
  not_covered: string[];
}

export interface Attachment {
  kind: string;
  title: string;
  body: string;
}

export interface EvidenceItem {
  id: string;
  row: string;
  claim: string;
  classifications: Classification[];
  evidence: string;
  method: string;
  not_checked: string[];
  establishes: string;
  remaining_uncertainty: string;
  attachments: Attachment[];
  concerns: Concern;
}

export interface TechnicalVerdict {
  checked_passed: string[];
  checked_failed: string[];
  simulated: string[];
  all_checked_passed: boolean;
  statement: string;
}

export interface EvidencePackage {
  id: string;
  scenario_id: string;
  revision: number;
  title: string;
  problem: string;
  artifact: Artifact;
  claimed_commitment: string;
  items: EvidenceItem[];
  checks: CheckResult[];
  technical_verdict: TechnicalVerdict;
  desirability: string;
  specification_boundary: string[];
  claims_complete_correctness: boolean;
  simulation_scope: string;
  decision_requested: string;
  fixture_note: string;
}

export interface PeriodStat {
  period: number;
  claims: number;
  paid: number;
  rejected: number;
  gross: number;
  net: number;
  fee: number;
  duplicates: number;
}

export interface DuplicatePayout {
  wallet: number;
  block_period: number;
  first_block: number;
  second_block: number;
  amount_gross: number;
}

export interface ReplayStats {
  version: string;
  impl_id: string;
  stream: string;
  claims: number;
  paid: number;
  rejected: number;
  misreads: number;
  duplicate_count: number;
  duplicates: DuplicatePayout[];
  affected_wallets: number;
  total_gross: number;
  total_net: number;
  total_fee: number;
  diverted: number;
  overpayment_gross: number;
  overpayment_share_bps: number;
  average_daily_gross: number;
  per_period: PeriodStat[];
  first_block: number;
  last_block: number;
  migrated_entries: number;
}

export interface FuzzReport {
  property: string;
  version: string;
  runs: number;
  claims: number;
  violations: number;
  runs_with_violation: number;
  model: string;
}

export interface ReplaySummary {
  narrative: string;
  production: ReplayStats;
  candidate: ReplayStats;
  fuzz_current: FuzzReport;
  fuzz_candidate: FuzzReport;
  differing_wallets: number;
  stream_description: string;
}

export interface EconRow {
  group: string;
  before: number;
  after: number;
  delta: number;
  delta_bps: number;
}

export interface EconomicComparison {
  parameter: string;
  before: number;
  after: number;
  periods: number;
  rows: EconRow[];
  transfers_under_new_value: number;
  note: string;
}

export interface Guardian {
  id: string;
  label: string;
  public_key_hex: string;
}

export interface GuardianSet {
  guardians: Guardian[];
  note: string;
}

export interface QuorumPolicy {
  id: string;
  label: string;
  m: number;
  n: number;
  description: string;
  assumptions: string[];
  is_default: boolean;
}

export interface Approval {
  guardian_id: string;
  proposal_id: string;
  revision: number;
  decision: Decision;
  commitment: string;
  note: string;
  surface_mode_viewed: SurfaceMode;
  signature_hex: string;
}

export interface Authorization {
  proposal_id: string;
  revision: number;
  commitment: string;
  policy: QuorumPolicy;
  approvals: Approval[];
  issued_seq: number;
}

export interface VerifierStep {
  name: string;
  ok: boolean;
  detail: string;
}

export interface SignatureCheck {
  guardian_id: string;
  decision: Decision;
  commitment: string;
  ok: boolean;
  detail: string;
}

export interface VerifierReport {
  verdict: Verdict;
  computed_commitment: string | null;
  expected_commitment: string | null;
  manifest_sha256: string;
  payload_sha256: string;
  manifest_canonical_len: number;
  payload_len: number;
  steps: VerifierStep[];
  signatures: SignatureCheck[];
  quorum_have: number;
  quorum_need: number;
  reasons: string[];
  what_this_establishes: string;
  what_this_does_not_establish: string;
}

export type ExecutionRefusal =
  | { kind: "commitment_mismatch"; expected: string; computed: string; verifier: VerifierReport }
  | { kind: "authorization_invalid"; verifier: VerifierReport }
  | { kind: "wrong_proposal"; expected: string; got: string }
  | { kind: "payload_rejected"; reason: string };

export interface InstallReport {
  from_version: string;
  to_version: string;
  migrated_entries: number;
  format_version: number;
}

export interface PostStateVerification {
  checks: CheckResult[];
  smoke_replay: string;
  ok: boolean;
  escalation: string | null;
}

export interface ExecutionReport {
  proposal_id: string;
  commitment: string;
  verifier: VerifierReport;
  install: InstallReport;
  block: number;
  post_state: PostStateVerification;
  statement: string;
}

export type ExecutionOutcome = { outcome: "applied"; report: ExecutionReport } | { outcome: "refused"; refusal: ExecutionRefusal };

export interface TamperRecord {
  variant: TamperVariant;
  description: string;
  after_status: ProposalStatus;
  original_commitment: string;
  tampered_commitment: string;
}

export interface LayerResult {
  layer: number;
  title: string;
  question: string;
  status: LayerStatus;
  finding: string;
  details: string[];
}

export interface RecoveryAction {
  description: string;
  privileged: boolean;
  authorized_by: string;
}

export interface RecoveryAssessment {
  upgrade: string | null;
  layers: LayerResult[];
  automatic_recovery_blocked: boolean;
  code_revert_is_recovery: boolean;
  why_code_revert_insufficient: string[];
  requires_new_authorization: boolean;
  actions: RecoveryAction[];
  misreads_if_reverted: number;
  statement: string;
}

export interface RevertAttempt {
  blocked: boolean;
  reason: string;
  assessment: RecoveryAssessment;
  draft: Artifact | null;
}

export interface IncidentPolicy {
  id: string;
  label: string;
  pre_authorized: Rung | null;
  max_duration_blocks: number;
  lift_requires_guardians: boolean;
  assumptions: string[];
  is_default: boolean;
  hypothetical: boolean;
}

export interface IncidentParams {
  exploit_loss_per_block: number;
  legit_single_claims_per_block: number;
  legit_batch_claims_per_block: number;
  exits_per_block: number;
  integrations_affected_by_pause: number;
  agent_detection_block: number;
  quorum_arrival_block: number;
  horizon_blocks: number;
  false_positive: boolean;
}

export interface Interrupted {
  single_claims: number;
  batch_claims: number;
  exits: number;
  integrations: number;
  distribution_blocks_halted: number;
}

export interface TimelineEntry {
  block: number;
  actor: string;
  event: string;
}

export interface IncidentOutcome {
  policy: IncidentPolicy;
  false_positive: boolean;
  loss_before_containment: number;
  loss_after_containment: number;
  containment_block: number | null;
  containment_action: Rung | null;
  fix_block: number;
  lifted_block: number | null;
  interrupted: Interrupted;
  timeline: TimelineEntry[];
  authority_notes: string[];
  side_effects: string;
}

export interface EmergencyGrant {
  incident_id: string;
  policy_id: string;
  rung: Rung;
  granted_at_block: number;
  expires_at_block: number | null;
  used: boolean;
  lifted_by_guardians: boolean;
}

export interface IncidentState {
  incident_id: string;
  open: boolean;
  policy_id: string;
  params: IncidentParams;
  outcomes: IncidentOutcome[];
  grant: EmergencyGrant | null;
  log: string[];
}

export interface CompletedProposal {
  proposal_id: string;
  commitment: string | null;
  status: ProposalStatus;
  summary: string;
}

export interface ExternalEffect {
  kind: EffectKind;
  block: number;
  description: string;
  amount: number | null;
  count: number;
}

export interface UpgradeSummary {
  proposal_id: string;
  from_version: string;
  to_version: string;
  commitment: string;
  commitment_short: string;
  block: number;
}

export interface StateSummary {
  block: number;
  period: number;
  version: string;
  format_version: number;
  fee_bps: number;
  paused: boolean;
  pause_detail: string;
  payout_count: number;
  duplicates: number;
  treasury: number;
  fee_collected: number;
  diverted: number;
  wallets_with_history: number;
  effects: ExternalEffect[];
  history: UpgradeSummary[];
  functions: string[];
}

export interface ScenarioView {
  key: string;
  meta: ScenarioMeta;
  proposal_id: string;
  revision: number;
  status: ProposalStatus;
  status_label: string;
  stage: string;
  state: StateSummary;
  observation: Observation | null;
  detection: Detection | null;
  diagnosis: string | null;
  candidate: ArtifactView | null;
  candidate_diff: string | null;
  displayed_summary: DisplayedSummary | null;
  checks: CheckResult[];
  replay: ReplaySummary | null;
  economics: EconomicComparison | null;
  package: EvidencePackage | null;
  surface_mode: SurfaceMode;
  guardians: GuardianSet;
  quorum_policy: QuorumPolicy;
  approvals: Approval[];
  invalidated_approvals: number;
  authorization: Authorization | null;
  execution: ExecutionOutcome | null;
  tamper: TamperRecord | null;
  recovery: RecoveryAssessment | null;
  revert_attempt: RevertAttempt | null;
  incident: IncidentState | null;
  completed: CompletedProposal[];
  narrative: string[];
  next_agent_step: AgentStep | null;
}

export interface SurfaceSection {
  title: string;
  lines: string[];
  item_ids: string[];
}

export interface SurfaceView {
  mode: SurfaceMode;
  label: string;
  description: string;
  commitment: string;
  headline: string;
  sections: SurfaceSection[];
  omitted: string[];
  attachments: Attachment[];
  items_shown: number;
  items_total: number;
  note: string;
}

export interface SurfaceComparison {
  commitment_identical: boolean;
  package_unchanged: boolean;
  views: SurfaceView[];
  working_guess: string;
  open_question: string;
}

export interface RecomputeResult {
  computed: string;
  claimed: string;
  matches: boolean;
  manifest_canonical_len: number;
  payload_len: number;
}

export interface GuardianOutcome {
  approval: Approval;
  status: ProposalStatus;
  authorization: Authorization | null;
  quorum_note: string;
}

export interface ScenarioSummary {
  key: string;
  title: string;
  package_title: string;
  status: ProposalStatus;
  status_label: string;
  stage: string;
  commitment_short: string | null;
  tampered: boolean;
}

export interface ReviewNote {
  id: number;
  created_seq: number;
  observed_problem: string;
  scenario: string;
  technical_concern: string;
  suggested_policy_change: string;
  remaining_question: string;
  question_ref: number | null;
  created_at: string | null;
}

export interface AuditEvent {
  seq: number;
  scenario: string | null;
  actor: string;
  action: string;
  detail: string;
  commitment: string | null;
  timestamp: string | null;
}

export interface FailureModeInfo {
  id: string;
  key: string;
  title: string;
  description: string;
  needs: string;
  reference: string | null;
  scenario: string;
  question: number;
}

export interface FailureModeResult {
  id: string;
  key: string;
  title: string;
  expected: string;
  observed: string;
  holds: boolean;
  steps: string[];
  evidence: unknown;
}

export interface QuestionLink {
  label: string;
  route: string;
  scenario: string | null;
  failure_mode: string | null;
}

export interface ReviewQuestion {
  number: number;
  text: string;
  concerns: string;
  links: QuestionLink[];
  status: string;
}

export interface RungInfo {
  rung: Rung;
  number: number;
  label: string;
  changes_protocol_state: boolean;
  side_effects: string;
}

export interface Settings {
  dark_mode: boolean;
}

export interface WorkspaceView {
  app_version: string;
  scope_statement: string;
  scenarios: ScenarioSummary[];
  notes: ReviewNote[];
  audit: AuditEvent[];
  failure_modes: FailureModeInfo[];
  failure_results: FailureModeResult[];
  questions: ReviewQuestion[];
  quorum_presets: QuorumPolicy[];
  incident_policies: IncidentPolicy[];
  rungs: RungInfo[];
  settings: Settings;
  hypothesis: string[];
  assumptions: string[];
}

export interface ActionResult {
  ok: boolean;
  message: string;
  scenario: ScenarioView;
}

export interface ExportResult {
  directory: string;
  files: string[];
}
