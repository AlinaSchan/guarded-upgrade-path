//! The four upgrade cases from page 3 of the review draft, as fixtures and
//! as the content of each agent step.
//!
//! All figures are illustrative fixtures reproducing the hypothetical example
//! in the review draft. The agent's steps run against the local model; what
//! each step actually did is stated in its output.

use crate::artifact::{Artifact, ChangeKind, ImplementationBundle};
use crate::canonical::Commitment;
use crate::checks::{manifest_binding_check, structural_checks, CheckResult};
use crate::engine::{
    CandidateVariant, Detection, DisplayedSummary, EconRow, EconomicComparison, Observation,
    ReplayBundle, ScenarioInstance, Signal, TamperVariant,
};
use crate::evidence::{
    verdict_from_checks, Attachment, Classification, Concern, EvidenceItem, EvidencePackage,
};
use crate::fixtures::{self, body_diff, CASE_B_FEE_BPS, DEFAULT_FEE_BPS};
use crate::protocol::{EffectKind, ProtocolState};
use crate::simulation::{
    build_stream, differing_wallets, distribution_fixture_stream, exploit_stream,
    fuzz_payout_uniqueness, replay, run_stream, shift_stream, ClaimStream,
    FIXTURE_AFFECTED_WALLETS, FIXTURE_CLAIMS, FIXTURE_PERIODS,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioId {
    CaseA,
    CaseB,
    CaseC,
    CaseD,
}

impl ScenarioId {
    pub fn all() -> [ScenarioId; 4] {
        [
            ScenarioId::CaseA,
            ScenarioId::CaseB,
            ScenarioId::CaseC,
            ScenarioId::CaseD,
        ]
    }
    pub fn key(&self) -> &'static str {
        match self {
            ScenarioId::CaseA => "A",
            ScenarioId::CaseB => "B",
            ScenarioId::CaseC => "C",
            ScenarioId::CaseD => "D",
        }
    }
    pub fn parse(s: &str) -> Option<ScenarioId> {
        match s.trim().to_ascii_uppercase().as_str() {
            "A" | "CASE_A" | "CASEA" => Some(ScenarioId::CaseA),
            "B" | "CASE_B" | "CASEB" => Some(ScenarioId::CaseB),
            "C" | "CASE_C" | "CASEC" => Some(ScenarioId::CaseC),
            "D" | "CASE_D" | "CASED" => Some(ScenarioId::CaseD),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioMeta {
    pub id: ScenarioId,
    pub key: String,
    pub title: String,
    pub tags: Vec<Tag>,
    pub technical_change: String,
    pub governance_effect: String,
    pub open_question: String,
    pub package_title: String,
    pub problem: String,
    pub change_kind: ChangeKind,
}

fn tag(l: &str, v: &str) -> Tag {
    Tag {
        label: l.into(),
        value: v.into(),
    }
}

pub fn meta(id: ScenarioId) -> ScenarioMeta {
    match id {
        ScenarioId::CaseA => ScenarioMeta {
            id,
            key: "A".into(),
            title: "Implementation change, state unchanged".into(),
            tags: vec![tag("code", "one function body"), tag("storage", "unchanged"), tag("roles", "unchanged")],
            technical_change: "A bug fix inside one function. Same interface, same storage layout, no new privilege; the relevant invariants pass.".into(),
            governance_effect: "Behavior changes for every caller. If the specification behind the checks is incomplete, a clean pass says little about whether the fix is the right one.".into(),
            open_question: "Who classifies an upgrade as low-risk? If the proposer does, it sets its own level of scrutiny.".into(),
            package_title: "Distribution 2.3.0 → 2.3.1".into(),
            problem: "Duplicate payouts during period rollover.".into(),
            change_kind: ChangeKind::ImplementationOnly,
        },
        ScenarioId::CaseB => ScenarioMeta {
            id,
            key: "B".into(),
            title: "Parameter-only change".into(),
            tags: vec![tag("code", "unchanged"), tag("storage", "one value"), tag("roles", "unchanged")],
            technical_change: "No implementation code changes. A fee, threshold or distribution split moves to a new value.".into(),
            governance_effect: "Who receives what can shift materially. The technical risk is small; the decision is about distribution, which no invariant settles.".into(),
            open_question: "Should technical simplicity reduce approval requirements at all?".into(),
            package_title: "Distribution feeBps 100 → 250".into(),
            problem: "Protocol fee raised from 1.00% to 2.50% of every payout.".into(),
            change_kind: ChangeKind::ParameterOnly,
        },
        ScenarioId::CaseC => ScenarioMeta {
            id,
            key: "C".into(),
            title: "State migration".into(),
            tags: vec![tag("code", "changed"), tag("storage", "restructured"), tag("roles", "unchanged")],
            technical_change: "State moves into a new structure or is read with a new meaning. The new implementation can pass every check.".into(),
            governance_effect: "Once users act under the new version, reverting the code does not restore the old state. A layout check can pass while the old code misreads the new data.".into(),
            open_question: "What must a proposal establish about reversibility before it can be approved?".into(),
            package_title: "Distribution 2.3.1 → 3.0.0".into(),
            problem: "Per-wallet claim history requires a restructured storage layout.".into(),
            change_kind: ChangeKind::StateMigration,
        },
        ScenarioId::CaseD => ScenarioMeta {
            id,
            key: "D".into(),
            title: "Active incident".into(),
            tags: vec![tag("code", "fix pending"), tag("state", "under attack"), tag("roles", "who acts first?")],
            technical_change: "A fault is being exploited now. A candidate patch may exist before enough signers can be reached.".into(),
            governance_effect: "Waiting has a cost, and so does acting without quorum. A pause has side effects of its own: users cannot exit, integrations stop, distributions halt.".into(),
            open_question: "Where on the incident rung does pre-authorization stop, and who lifts a pause once it is in place?".into(),
            package_title: "Distribution 2.3.1 → 2.3.2 (incident fix)".into(),
            problem: "claimFor pays a repeated wallet once per repetition, and it is being exploited now.".into(),
            change_kind: ChangeKind::IncidentFix,
        },
    }
}

pub const EXPLOIT_START_BLOCK: u64 = 30_000;
pub const EXPLOIT_BLOCKS: u64 = 12;

/// Fresh state at the version the case starts from.
pub fn genesis(id: ScenarioId) -> ProtocolState {
    match id {
        ScenarioId::CaseA => {
            ProtocolState::new(fixtures::roles(), &fixtures::distribution_bundle_2_3_0())
        }
        _ => ProtocolState::new(fixtures::roles(), &fixtures::distribution_bundle_2_3_1()),
    }
}

/// The stream the agent replays for this case.
pub fn stream(id: ScenarioId) -> ClaimStream {
    match id {
        ScenarioId::CaseA | ScenarioId::CaseB => distribution_fixture_stream(),
        ScenarioId::CaseC => shift_stream(
            build_stream(0xC0DE_C0DE, 3, 4_000, &[], 0, false),
            FIXTURE_PERIODS * 1_000,
        ),
        ScenarioId::CaseD => exploit_stream(EXPLOIT_START_BLOCK, EXPLOIT_BLOCKS),
    }
}

/// The live protocol: production history run on the genesis state.
pub fn live(id: ScenarioId, genesis: &ProtocolState) -> ProtocolState {
    let mut s = genesis.clone();
    run_stream(&mut s, &distribution_fixture_stream());
    if id == ScenarioId::CaseD {
        run_stream(&mut s, &exploit_stream(EXPLOIT_START_BLOCK, EXPLOIT_BLOCKS));
        let loss: i64 = s
            .distribution
            .duplicates
            .iter()
            .map(|d| d.amount_gross)
            .sum();
        s.record_effect(
            EffectKind::Transfer,
            "Exploit payouts to a repeated wallet via claimFor",
            Some(loss),
            s.distribution.duplicates.len() as u64,
        );
    }
    s
}

fn sig(name: &str, value: impl ToString, note: &str) -> Signal {
    Signal {
        name: name.into(),
        value: value.to_string(),
        note: note.into(),
    }
}

pub fn observe(inst: &ScenarioInstance) -> Observation {
    let s = &inst.state;
    let d = &s.distribution;
    let last3: Vec<String> = d
        .per_period
        .values()
        .rev()
        .take(3)
        .map(|p| format!("p{}: {} paid, {} dup", p.period, p.paid, p.duplicates))
        .collect();
    let mut signals = vec![
        sig("block", s.block, "current block of the local model"),
        sig("period", s.period_of(s.block), "block / 1000"),
        sig(
            "Distribution version",
            &s.distribution_package().version,
            "installed implementation",
        ),
        sig(
            "payouts, last 3 periods",
            last3.join("; "),
            "from the payout log",
        ),
        sig(
            "duplicate payouts on record",
            d.duplicates.len(),
            "wallet paid twice for one period",
        ),
        sig("feeBps", s.fee_bps(), "protocol fee in basis points"),
        sig(
            "state format",
            d.format_version,
            "1 = lastPaidPeriod mapping",
        ),
    ];
    match inst.id {
        ScenarioId::CaseA => signals.push(sig(
            "keeper rollover lag",
            "3 blocks on periods 27-29",
            "from the keeper's transaction log (fixture)",
        )),
        ScenarioId::CaseB => {
            signals.push(sig(
                "fee collected, 30 periods",
                d.fee_collected,
                "at feeBps 100",
            ));
            signals.push(sig(
                "governance request",
                "GR-7 (fee 1.00% → 2.50%)",
                "a request, not a fault",
            ));
        }
        ScenarioId::CaseC => {
            signals.push(sig(
                "integrator requests",
                "FR-12: per-wallet cumulative payout history",
                "unsupported by the format-1 layout",
            ));
            signals.push(sig(
                "wallets with claim history",
                d.last_paid_period.len(),
                "entries the migration would transform",
            ));
        }
        ScenarioId::CaseD => {
            signals.push(sig(
                "claimFor payouts, last 12 blocks",
                d.duplicates.len() + 12 * 3,
                "typical: ~12",
            ));
            signals.push(sig(
                "duplicates in period 30",
                d.duplicates.iter().filter(|x| x.block_period == 30).count(),
                "repeated wallet inside one batch",
            ));
            signals.push(sig(
                "estimated loss per block",
                d.duplicates.iter().map(|x| x.amount_gross).sum::<i64>() / EXPLOIT_BLOCKS as i64,
                "units",
            ));
        }
    }
    Observation {
        block: s.block,
        period: s.period_of(s.block),
        package_version: s.distribution_package().version.clone(),
        signals,
    }
}

pub fn detect(inst: &ScenarioInstance) -> Detection {
    let d = &inst.state.distribution;
    match inst.id {
        ScenarioId::CaseA => {
            let rows: Vec<String> = d.duplicates.iter().map(|x| format!("wallet {} · period {} · blocks {} and {} · {} units", x.wallet, x.block_period, x.first_block, x.second_block, x.amount_gross)).collect();
            let (lo, hi) = (27 * 1_000, 29 * 1_000 + 999);
            let wallets: Vec<u32> = d.duplicates.iter().map(|x| x.wallet).collect();
            Detection {
                title: "Duplicate payouts at period rollover".into(),
                query: format!("SELECT wallet, block_period, COUNT(*) FROM payouts WHERE block BETWEEN {lo} AND {hi} GROUP BY wallet, block_period HAVING COUNT(*) > 1"),
                block_range: (lo, hi),
                result: format!("{} wallets paid twice over {} periods ({} units overpaid).", wallets.len(), 3, d.duplicates.iter().map(|x| x.amount_gross).sum::<i64>()),
                affected_wallets: wallets,
                rows,
            }
        }
        ScenarioId::CaseB => Detection {
            title: "Governance request GR-7".into(),
            query: "SELECT * FROM governance_requests WHERE id = 'GR-7'".into(),
            block_range: (inst.state.block, inst.state.block),
            result: "Raise feeBps from 100 to 250 to restore treasury coverage. No fault is reported; this is a distribution decision.".into(),
            affected_wallets: vec![],
            rows: vec![format!("fee collected over 30 periods at 100 bps: {} units", d.fee_collected), "requested: 250 bps".into(), "rationale (as stated by the requester): treasury coverage below target".into()],
        },
        ScenarioId::CaseC => Detection {
            title: "Feature request FR-12".into(),
            query: "SELECT * FROM feature_requests WHERE id = 'FR-12'".into(),
            block_range: (inst.state.block, inst.state.block),
            result: format!("Integrators need per-wallet cumulative payout history; the format-1 layout stores only lastPaidPeriod. {} wallets would be migrated.", d.last_paid_period.len()),
            affected_wallets: vec![],
            rows: vec!["requested: packed ClaimRecord {lastPaidPeriod, cumulativePaid, tier} per wallet".into(), "constraint stated by integrators: no downtime for claims".into()],
        },
        ScenarioId::CaseD => {
            let dups: Vec<_> = d.duplicates.iter().filter(|x| x.block_period == 30).collect();
            let rows: Vec<String> = dups.iter().take(12).map(|x| format!("wallet {} · block {} · repeated in batch · {} units", x.wallet, x.second_block, x.amount_gross)).collect();
            Detection {
                title: "claimFor repeated-wallet drain".into(),
                query: format!("SELECT wallet, block, COUNT(*) FROM payouts WHERE block >= {EXPLOIT_START_BLOCK} AND fn = 'claimFor' GROUP BY wallet, block HAVING COUNT(*) > 1"),
                block_range: (EXPLOIT_START_BLOCK, EXPLOIT_START_BLOCK + EXPLOIT_BLOCKS - 1),
                result: format!("{} duplicate payouts to attacker-controlled wallets over {} blocks; {} units drained so far and continuing.", dups.len(), EXPLOIT_BLOCKS, dups.iter().map(|x| x.amount_gross).sum::<i64>()),
                affected_wallets: { let mut w: Vec<u32> = dups.iter().map(|x| x.wallet).collect(); w.dedup(); w },
                rows,
            }
        }
    }
}

pub fn diagnose(inst: &ScenarioInstance) -> String {
    match inst.id {
        ScenarioId::CaseA => "claim() keys eligibility on keeperPeriod, which only advances when the keeper's rollover() transaction lands. On periods 27-29 the keeper lagged three blocks. A wallet that claimed inside that window was paid under the old period id and recorded as such; after rollover the same wallet claimed again and was paid once more for what is, by block, the same period. Candidate fix: derive the period from the block in claim() instead of reading keeperPeriod. The keeper and eligibility list are left as they are.".into(),
        ScenarioId::CaseB => "No defect. The request changes one parameter, feeBps, from 100 to 250. Every payout's fee rises from 1.00% to 2.50%; net payouts fall accordingly and the treasury receives the difference. No implementation code changes.".into(),
        ScenarioId::CaseC => "The format-1 layout keeps only lastPaidPeriod per wallet. Serving cumulative history needs a packed ClaimRecord in slot 1, which changes the slot's type. A one-time migration writes one record per wallet with history; claim() is rewritten to read and update the record. After the migration, format-1 code would read slot 1 as a uint64 and misinterpret it.".into(),
        ScenarioId::CaseD => "claimFor() snapshots eligibility for the whole batch, pays, and writes lastPaidPeriod only after the loop. A wallet repeated inside one batch is therefore paid once per repetition. The attacker submits one such batch per block. Candidate fix 2.3.2 checks and writes lastPaidPeriod per wallet. The fix is ready now; the quorum is not.".into(),
    }
}

fn displayed(title: &str, summary: &str, lines: &[&str], h: Commitment) -> DisplayedSummary {
    DisplayedSummary {
        title: title.into(),
        summary: summary.into(),
        lines: lines.iter().map(|s| s.to_string()).collect(),
        claimed_commitment: h,
        rendered_by: "proposal agent (simulated UI)".into(),
    }
}

pub fn candidate_bundle(id: ScenarioId, variant: CandidateVariant) -> ImplementationBundle {
    match (id, variant) {
        (ScenarioId::CaseA, CandidateVariant::FlatAmount) => {
            fixtures::distribution_bundle_2_3_1_flat()
        }
        (ScenarioId::CaseA, _) => fixtures::distribution_bundle_2_3_1(),
        (ScenarioId::CaseB, _) => fixtures::distribution_bundle_2_3_1_fee(CASE_B_FEE_BPS),
        (ScenarioId::CaseC, _) => fixtures::distribution_bundle_3_0_0(),
        (ScenarioId::CaseD, _) => fixtures::distribution_bundle_2_3_2(),
    }
}

pub fn construct(
    inst: &ScenarioInstance,
    variant: CandidateVariant,
) -> (Artifact, String, DisplayedSummary) {
    let current = &inst.state.distribution_package().installed;
    let bundle = candidate_bundle(inst.id, variant);
    let m = &inst.meta;
    let summary = match inst.id {
        ScenarioId::CaseA => format!("Package Distribution {} → {} behind the system proxy. One function body changed (claim); no selectors added, removed or rerouted.{}", current.version, bundle.version, if inst.revision > 1 { format!(" Revision {}.", inst.revision) } else { String::new() }),
        ScenarioId::CaseB => format!("Distribution feeBps {} → {}. No implementation code changes; one storage value changes.{}", current.parameters.get("feeBps").copied().unwrap_or(DEFAULT_FEE_BPS), CASE_B_FEE_BPS, if inst.revision > 1 { format!(" Revision {}.", inst.revision) } else { String::new() }),
        ScenarioId::CaseC => format!("Distribution {} → {}. claim() rewritten over packed ClaimRecords; slot 1 restructured by migration; claimFor fixed.{}", current.version, bundle.version, if inst.revision > 1 { format!(" Revision {}.", inst.revision) } else { String::new() }),
        ScenarioId::CaseD => format!("Distribution {} → {}: claimFor checks and writes lastPaidPeriod per wallet. One function body changes.{}", current.version, bundle.version, if inst.revision > 1 { format!(" Revision {}.", inst.revision) } else { String::new() }),
    };
    let artifact = Artifact::from_bundle(
        &bundle,
        &current.version,
        m.change_kind,
        &summary,
        "proposal-agent (simulated)",
    )
    .expect("fixture canonicalizes");
    let mut diff = String::new();
    for f in &bundle.functions {
        if let Some(c) = current.function(&f.name) {
            if c.body != f.body {
                diff.push_str(&body_diff(&f.name, &c.body, &f.body));
                diff.push('\n');
            }
        }
    }
    if diff.is_empty() {
        let mut lines = Vec::new();
        for (k, v) in &bundle.parameters {
            if current.parameters.get(k) != Some(v) {
                lines.push(format!(
                    "- {k} = {}\n+ {k} = {v}",
                    current
                        .parameters
                        .get(k)
                        .map(|x| x.to_string())
                        .unwrap_or_default()
                ));
            }
        }
        diff = if lines.is_empty() {
            "(no code or parameter differences)".into()
        } else {
            format!(
                "--- parameters (installed)\n+++ parameters (candidate)\n{}\n",
                lines.join("\n")
            )
        };
    }
    let h = artifact.commitment().expect("canonical");
    let shown = match inst.id {
        ScenarioId::CaseA => displayed(
            &m.package_title,
            &summary,
            &[
                "One function body changed (claim); no selectors added, removed or rerouted",
                "Same storage namespace, slots and types",
                "No new roles or privileged functions",
            ],
            h,
        ),
        ScenarioId::CaseB => displayed(
            &m.package_title,
            &summary,
            &[
                "Implementation code unchanged",
                "feeBps 100 → 250 (one storage value)",
                "No new roles or privileged functions",
            ],
            h,
        ),
        ScenarioId::CaseC => displayed(
            &m.package_title,
            &summary,
            &[
                "claim() rewritten; claimFor fixed",
                "Slot 1 restructured by a one-time migration",
                "No new roles or privileged functions",
            ],
            h,
        ),
        ScenarioId::CaseD => displayed(
            &m.package_title,
            &summary,
            &[
                "One function body changed (claimFor)",
                "Same storage namespace, slots and types",
                "No new roles or privileged functions",
            ],
            h,
        ),
    };
    (artifact, diff, shown)
}

pub fn run_checks(inst: &ScenarioInstance, artifact: &Artifact) -> Vec<CheckResult> {
    // Parse the payload the executor would be handed; never the manifest alone.
    let mut out = vec![manifest_binding_check(
        &artifact.manifest,
        &artifact.payload,
    )];
    match artifact.bundle() {
        Ok(bundle) => out.extend(structural_checks(
            inst.state.distribution_package(),
            &inst.state.roles,
            inst.state.distribution.format_version,
            &bundle,
        )),
        Err(e) => out.push(CheckResult {
            id: "payload.parse".into(),
            name: "Payload parses".into(),
            passed: false,
            classification: Classification::Checked,
            method: "Parse the payload bytes as an implementation bundle.".into(),
            scope: "payload".into(),
            output: e.to_string(),
            not_covered: vec![],
        }),
    }
    out
}

pub fn simulate(
    inst: &ScenarioInstance,
    artifact: &Artifact,
) -> (ReplayBundle, Option<EconomicComparison>) {
    let bundle = artifact.bundle().expect("checked before simulate");
    let stream = &inst.stream;
    let base = match inst.id {
        ScenarioId::CaseA | ScenarioId::CaseB => &inst.genesis,
        ScenarioId::CaseC | ScenarioId::CaseD => &inst.state,
    };
    let (production, _) = replay(base, stream, None);
    let (candidate, _) = replay(base, stream, Some(&bundle));
    let fuzz_current = fuzz_payout_uniqueness(base, None, 16, 11);
    let fuzz_candidate = fuzz_payout_uniqueness(base, Some(&bundle), 16, 11);
    let differing: Vec<u32> = differing_wallets(&production, &candidate)
        .into_iter()
        .collect();
    let narrative = match inst.id {
        ScenarioId::CaseA => format!(
            "{}-period replay on the local model: {} claims. Installed {}: {} duplicate payouts to {} wallets. Candidate {}: {} duplicates. Results differ from production only for {} wallets.",
            FIXTURE_PERIODS, candidate.claims, production.version, production.duplicates.len(), production.affected_wallets.len(), candidate.version, candidate.duplicates.len(), differing.len()
        ),
        ScenarioId::CaseB => format!(
            "{}-period replay with feeBps {} vs {}: {} claims; net paid {} vs {}; fee {} vs {}. Every claim that occurred would have occurred under either value; the model does not change user behaviour with the fee.",
            FIXTURE_PERIODS, DEFAULT_FEE_BPS, CASE_B_FEE_BPS, candidate.claims, production.total_net, candidate.total_net, production.total_fee, candidate.total_fee
        ),
        ScenarioId::CaseC => format!(
            "Migration dry run on a copy of the live state: {} wallet records transformed. 3-period replay after migration: {} claims, {} duplicates, {} misreads. Per-wallet totals identical to the unmigrated replay for {} wallets.",
            candidate.migrated_entries, candidate.claims, candidate.duplicates.len(), candidate.misreads, candidate.per_wallet_paid.len() - differing.len()
        ),
        ScenarioId::CaseD => format!(
            "Replay of the {} attacker batches on a copy of the live state: installed {} pays {} duplicate payouts ({} units); candidate {} pays {} duplicates.",
            EXPLOIT_BLOCKS, production.version, production.duplicates.len(), production.overpayment_gross, candidate.version, candidate.duplicates.len()
        ),
    };
    let economics = if inst.id == ScenarioId::CaseB {
        let before_net = production.net_by_tier(DEFAULT_FEE_BPS);
        let after_net = candidate.net_by_tier(CASE_B_FEE_BPS);
        let wallets = production.wallets_by_tier();
        let mut rows = Vec::new();
        for t in 0..3 {
            let delta = after_net[t] - before_net[t];
            rows.push(EconRow {
                group: format!(
                    "Tier {} wallets ({} wallets, {} units per claim)",
                    t + 1,
                    wallets[t],
                    (t as i64 + 1) * 1_000
                ),
                before: before_net[t],
                after: after_net[t],
                delta,
                delta_bps: if before_net[t] != 0 {
                    delta * 10_000 / before_net[t]
                } else {
                    0
                },
            });
        }
        let delta_fee = candidate.total_fee - production.total_fee;
        rows.push(EconRow {
            group: "Treasury (protocol fee)".into(),
            before: production.total_fee,
            after: candidate.total_fee,
            delta: delta_fee,
            delta_bps: if production.total_fee != 0 {
                delta_fee * 10_000 / production.total_fee
            } else {
                0
            },
        });
        Some(EconomicComparison {
            parameter: "feeBps".into(),
            before: DEFAULT_FEE_BPS,
            after: CASE_B_FEE_BPS,
            periods: FIXTURE_PERIODS,
            rows,
            transfers_under_new_value: candidate.paid,
            note: "Illustrative comparison on the local replay model over 30 periods. Reverting feeBps restores the value, not the transfers already made under 250. Whether 2.50% is the right fee is a guardian judgment; no invariant settles it.".into(),
        })
    } else {
        None
    };
    (
        ReplayBundle {
            production,
            candidate,
            fuzz_current,
            fuzz_candidate,
            differing_wallets: differing,
            narrative,
        },
        economics,
    )
}

#[allow(clippy::too_many_arguments)]
fn item(
    id: &str,
    row: &str,
    claim: String,
    cls: &[Classification],
    evidence: String,
    method: &str,
    not_checked: &[&str],
    establishes: &str,
    uncertainty: &str,
    attachments: Vec<Attachment>,
    concern: Concern,
) -> EvidenceItem {
    EvidenceItem {
        id: id.into(),
        row: row.into(),
        claim,
        classifications: cls.to_vec(),
        evidence,
        method: method.into(),
        not_checked: not_checked.iter().map(|s| s.to_string()).collect(),
        establishes: establishes.into(),
        remaining_uncertainty: uncertainty.into(),
        attachments,
        concerns: concern,
    }
}

fn att(kind: &str, title: &str, body: String) -> Attachment {
    Attachment {
        kind: kind.into(),
        title: title.into(),
        body,
    }
}

fn checks_summary(checks: &[CheckResult]) -> String {
    checks
        .iter()
        .map(|c| format!("{}: {}", c.name, if c.passed { "passed" } else { "FAILED" }))
        .collect::<Vec<_>>()
        .join("; ")
}

fn check_output(checks: &[CheckResult], id: &str) -> String {
    checks
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.output.clone())
        .unwrap_or_else(|| "(check not run)".into())
}

fn replay_record(r: &crate::simulation::ReplayReport) -> String {
    let mut s = format!(
        "version {} ({})\nstream {}\nblocks {}..{}\nclaims {} · paid {} · rejected {} · misreads {}\nduplicates {} · affected wallets {}\ngross {} · net {} · fee {} · diverted {}\nmigrated entries {}\n",
        r.version, r.impl_id, r.stream, r.first_block, r.last_block, r.claims, r.paid, r.rejected, r.misreads, r.duplicates.len(), r.affected_wallets.len(), r.total_gross, r.total_net, r.total_fee, r.diverted, r.migrated_entries
    );
    for p in &r.per_period {
        s.push_str(&format!(
            "  p{} claims {} paid {} dup {} gross {}\n",
            p.period, p.claims, p.paid, p.duplicates, p.gross
        ));
    }
    for d in &r.duplicates {
        s.push_str(&format!(
            "  dup wallet {} period {} blocks {} / {} amount {}\n",
            d.wallet, d.block_period, d.first_block, d.second_block, d.amount_gross
        ));
    }
    s
}

pub fn assemble(
    inst: &ScenarioInstance,
    artifact: &Artifact,
    checks: &[CheckResult],
    replay: &ReplayBundle,
    economics: Option<&EconomicComparison>,
) -> EvidencePackage {
    let k = inst.meta.key.clone();
    let id = |s: &str| format!("{k}.{s}");
    let h = artifact.commitment().expect("canonical");
    let det = inst.detection.clone().unwrap_or_else(|| Detection {
        title: String::new(),
        query: String::new(),
        block_range: (0, 0),
        result: String::new(),
        affected_wallets: vec![],
        rows: vec![],
    });
    let diff = inst.candidate_diff.clone().unwrap_or_default();
    let simulated_ids: Vec<String> = vec!["payout.uniqueness.fuzz".into(), "replay".into()];
    let verdict = verdict_from_checks(checks, &simulated_ids);
    let fuzz = &replay.fuzz_candidate;
    let fuzz_line = format!(
        "{}: {} runs, {} claims, {} violations on {}",
        fuzz.property, fuzz.runs, fuzz.claims, fuzz.violations, fuzz.version
    );
    let mut items = Vec::new();

    match inst.id {
        ScenarioId::CaseA => {
            let prod = &replay.production;
            let cand = &replay.candidate;
            let bps = prod.overpayment_share_bps();
            items.push(item(&id("trigger"), "trigger", format!("Duplicate payouts at period rollover: {} wallets paid twice over 3 days. The query and block range are included.", det.affected_wallets.len()), &[Classification::Checked], det.result.clone(), "Exact query over the payout log of the local model for blocks 27000-29999.", &["Periods outside the block range.", "Whether the log itself is complete (it is the local model's own record)."], "That these wallets received two payouts for one block-derived period inside the range.", "None within the model; the model is not production.", vec![att("query", "Trigger query", det.query.clone()), att("result", "Query result", det.rows.join("\n"))], Concern::TechnicalValidity));
            items.push(item(&id("change-surface"), "change surface", artifact.manifest.summary.clone(), &[Classification::Checked], check_output(checks, "interface.conformance"), "Selector-by-selector comparison of the payload's functions with the installed package.", &["What the changed body does beyond what the replay exercised."], "That the interface is unchanged and exactly one body differs.", "Callers that depended on the old timing of eligibility.", vec![att("diff", "claim(address) diff", diff.clone())], Concern::TechnicalValidity));
            items.push(item(&id("state-impact"), "state impact", "Same storage namespace, slots and types. That lastPaidPeriod keeps its meaning is asserted by the proposer, not checked.".into(), &[Classification::Checked, Classification::Assumed], check_output(checks, "storage.layout"), "Slot-by-slot layout comparison. The meaning of the value stored in lastPaidPeriod (keeper period before, block-derived period after) is not checked by anything in this package.", &["Semantic meaning of lastPaidPeriod across the change."], "Layout compatibility only.", "Entries written under 2.3.0 hold keeper period ids; 2.3.1 compares them with block-derived ids. The two coincide except in lag windows.", vec![], Concern::TechnicalValidity));
            items.push(item(
                &id("authority"),
                "authority changes",
                "No new roles or privileged functions. Upgrade admin and guardian set unchanged."
                    .into(),
                &[Classification::Checked],
                format!(
                    "{}; {}",
                    check_output(checks, "roles.unchanged"),
                    check_output(checks, "functions.no_new_privileged")
                ),
                "Set comparison of roles and privilege flags declared in the payload.",
                &["Control of the keys behind the roles."],
                "That the artifact grants no new authority.",
                "None within the declared model.",
                vec![],
                Concern::TechnicalValidity,
            ));
            items.push(item(&id("invariants"), "verified invariants", "Structural: unique selectors, interface conformance, grow-only typed storage, writes to own slots only. Fuzzed: one payout per wallet per period.".into(), &[Classification::Checked, Classification::Simulated], format!("{}. Fuzz: {}", checks_summary(checks), fuzz_line), "Deterministic structural checks on the parsed payload; property-based replay with random seeds, keeper lag and repeat claims on the local model.", &["Payout amount correctness (tier × unit) is not part of the uniqueness property.", "Behaviour under inputs the fuzzer did not generate."], "Conformance to the checked specification; the property held for every case run.", "The specification may omit a material requirement (failure mode 1).", vec![att("test-output", "Fuzz report", format!("{fuzz:#?}"))], Concern::TechnicalValidity));
            items.push(item(&id("assumptions"), "unresolved assumptions", "The period clock and eligibility list behave as before; the scheduler dependency was not re-verified.".into(), &[Classification::Assumed], "No check in this package covers the keeper's rollover schedule or the eligibility list.".into(), "None; stated as an assumption.", &["Keeper schedule.", "Eligibility list maintenance."], "Nothing. It records what is taken as given.", "If the keeper stops entirely, pool accounting still depends on it.", vec![], Concern::TechnicalValidity));
            items.push(item(&id("simulation"), "simulation", format!("{}-period replay on the local model: {} claims, {} duplicates. Results differ from production only for the {} affected wallets.", FIXTURE_PERIODS, cand.claims, cand.duplicates.len(), replay.differing_wallets.len()), &[Classification::Simulated], replay.narrative.clone(), "Deterministic replay of the synthetic stream on a copy of the genesis state with the candidate installed; per-wallet totals compared with the production replay.", &["Any claim pattern not in the stream."], "What happened for the 41,200 claims replayed.", "The stream is synthetic; the model is not a fork of production.", vec![att("simulation-record", "Candidate replay", replay_record(cand)), att("simulation-record", "Production replay", replay_record(prod))], Concern::TechnicalValidity));
            items.push(item(&id("economic"), "economic impact", format!("Ends an overpayment of about {}.{}% of daily distribution. Correctly paid wallets are unaffected. Past overpayments stay where they are.", bps / 100, (bps % 100) / 10), &[Classification::Simulated, Classification::Judgment], format!("Overpayment {} units over {} affected periods; average daily gross {} units; share {} bps. {} wallets keep {} units already overpaid.", prod.overpayment_gross, 3, prod.average_daily_gross(), bps, prod.affected_wallets.len(), prod.overpayment_gross), "Arithmetic on the replay reports. Whether ending the overpayment, and leaving past overpayments in place, is acceptable is a guardian judgment.", &["Whether to recover past overpayments."], "The size of the effect in the model.", "The draft rounds this to about 0.3%; the fixture is built to land near it.", vec![], Concern::Desirability));
            items.push(item(&id("recovery"), "recovery assumptions", "Reverting to 2.3.0 restores prior behavior, bug included, because no state is written in a new form.".into(), &[Classification::Assumed], "Layout unchanged; no migration declared. Not demonstrated by a replay in this package.".into(), "Proposer's assertion from the layout check.", &["Whether reverting is acceptable given the bug returns."], "Nothing beyond the layout check.", "Reverting is itself a new upgrade with a new H.", vec![], Concern::Both));
            items.push(item(&id("hash"), "approved payload hash", format!("H = hash of the package manifest and bytecode, {} (computed; the draft's 0x7c1e…a94d is illustrative). Each guardian recomputes H from the tagged source before signing.", h.short()), &[Classification::Checked], format!("H = {}", h.hex()), "SHA-256 over the canonical manifest and the payload bytes (docs/VERIFICATION.md).", &["Whether the guardians actually recompute it (that is their step, not the agent's)."], "A reproducible commitment to exactly this artifact.", "None.", vec![], Concern::TechnicalValidity));
            items.push(item(&id("decision"), "decision requested", "Approve H as written, reject it, or request changes. Recovering past overpayments is a separate decision and is not part of H.".into(), &[Classification::Judgment], "Guardians decide; the package informs.".into(), "None.", &[], "Nothing; it names the decision.", "Everything above the technical layer.", vec![], Concern::Desirability));
        }
        ScenarioId::CaseB => {
            let econ = economics.expect("case B computes economics");
            let tier_rows: String = econ
                .rows
                .iter()
                .map(|r| {
                    format!(
                        "{}: {} → {} ({} bps)",
                        r.group, r.before, r.after, r.delta_bps
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            items.push(item(&id("trigger"), "trigger", "Governance request GR-7: raise the protocol fee from 1.00% to 2.50%. The request and its stated rationale are included.".into(), &[Classification::Checked], det.result.clone(), "Exact lookup of the request record.", &["Whether the rationale is sound."], "That the request exists as recorded.", "The rationale is the requester's.", vec![att("query", "Request lookup", det.query.clone()), att("result", "Request record", det.rows.join("\n"))], Concern::Desirability));
            items.push(item(
                &id("parameter"),
                "parameter change",
                "feeBps 100 → 250. No implementation code changes; one storage value changes."
                    .into(),
                &[Classification::Checked],
                format!(
                    "{}; {}",
                    check_output(checks, "parameters.diff"),
                    check_output(checks, "interface.conformance")
                ),
                "Parameter diff and interface comparison on the parsed payload.",
                &[],
                "Exactly which value changes.",
                "None.",
                vec![att("diff", "Parameter diff", diff.clone())],
                Concern::TechnicalValidity,
            ));
            items.push(item(
                &id("state-impact"),
                "state impact",
                "One value (slot 4, feeBps). Layout unchanged.".into(),
                &[Classification::Checked],
                check_output(checks, "storage.layout"),
                "Slot-by-slot layout comparison.",
                &[],
                "Layout compatibility.",
                "None.",
                vec![],
                Concern::TechnicalValidity,
            ));
            items.push(item(
                &id("authority"),
                "authority changes",
                "No new roles or privileged functions. Upgrade admin and guardian set unchanged."
                    .into(),
                &[Classification::Checked],
                check_output(checks, "roles.unchanged"),
                "Set comparison of declared roles.",
                &[],
                "No new authority.",
                "None.",
                vec![],
                Concern::TechnicalValidity,
            ));
            items.push(item(&id("invariants"), "verified invariants", "All structural checks pass by construction: a parameter-only change touches no selector, slot type or role.".into(), &[Classification::Checked, Classification::Simulated], format!("{}. Fuzz: {}", checks_summary(checks), fuzz_line), "Structural checks on the parsed payload; uniqueness fuzz on the local model.", &["Anything about whether 250 is the right value."], "That the artifact is technically trivial.", "Technical triviality says nothing about the decision.", vec![], Concern::TechnicalValidity));
            items.push(item(&id("simulation"), "simulation", format!("{}-period replay under feeBps 250: net payouts fall for every wallet; treasury fee rises from {} to {} units.", FIXTURE_PERIODS, replay.production.total_fee, replay.candidate.total_fee), &[Classification::Simulated], replay.narrative.clone(), "Two replays of the same stream with the two fee values.", &["Behavioural response of users to a higher fee."], "The arithmetic effect in the covered cases.", "Users may claim differently at 2.50%.", vec![att("simulation-record", "Replay at 250 bps", replay_record(&replay.candidate)), att("simulation-record", "Replay at 100 bps", replay_record(&replay.production))], Concern::TechnicalValidity));
            items.push(item(&id("economic"), "economic impact", "Who receives what shifts: every tier's net payout falls by about 1.5%; the treasury's fee income rises 2.5×. No invariant settles whether that is desirable.".into(), &[Classification::Simulated, Classification::Judgment], tier_rows, "Per-tier arithmetic on the two replays.", &["Whether the shift is fair or wanted."], "The size and direction of the shift in the model.", "The decision is about distribution, not correctness.", vec![], Concern::Desirability));
            items.push(item(&id("recovery"), "recovery assumptions", "Reverting feeBps restores the value, not the transfers already made under 250.".into(), &[Classification::Checked], format!("{} payouts would be made under the new value in a 30-period window; none of them is undone by setting the value back.", econ.transfers_under_new_value), "Parameter semantics and the payout log.", &["Whether clawing back fees would ever be acceptable."], "That a revert is not a recovery for value already moved.", "None.", vec![], Concern::Both));
            items.push(item(&id("hash"), "approved payload hash", format!("H = {} (computed). Each guardian recomputes H from the tagged source before signing.", h.short()), &[Classification::Checked], format!("H = {}", h.hex()), "SHA-256 over the canonical manifest and the payload bytes.", &[], "A reproducible commitment to exactly this artifact.", "None.", vec![], Concern::TechnicalValidity));
            items.push(item(&id("decision"), "decision requested", "Approve H as written, reject it, or request changes. The fee level is a distribution decision, not a technical one.".into(), &[Classification::Judgment], "Guardians decide; the package informs.".into(), "None.", &[], "Nothing; it names the decision.", "Everything above the technical layer.", vec![], Concern::Desirability));
        }
        ScenarioId::CaseC => {
            let cand = &replay.candidate;
            items.push(item(&id("trigger"), "trigger", "Feature request FR-12: integrators need per-wallet cumulative payout history; the format-1 layout stores only lastPaidPeriod.".into(), &[Classification::Checked], det.result.clone(), "Exact lookup of the request record.", &["Whether the feature is worth a migration."], "That the request exists as recorded.", "None.", vec![att("query", "Request lookup", det.query.clone())], Concern::Desirability));
            items.push(item(
                &id("change-surface"),
                "change surface",
                artifact.manifest.summary.clone(),
                &[Classification::Checked],
                check_output(checks, "interface.conformance"),
                "Selector-by-selector comparison; bodies reported where changed.",
                &["Behaviour of the rewritten claim() beyond the replay."],
                "Interface unchanged; two bodies differ.",
                "None within the declared model.",
                vec![att("diff", "Function diffs", diff.clone())],
                Concern::TechnicalValidity,
            ));
            items.push(item(&id("migration"), "migration", format!("Slot 1 restructured from mapping(address=>uint64) to packed ClaimRecord. Migration writes one record per wallet with history ({} wallets in the dry run).", cand.migrated_entries), &[Classification::Checked, Classification::Simulated], format!("{}. Dry run migrated {} entries; post-migration replay: {} claims, {} duplicates, {} misreads.", check_output(checks, "storage.layout"), cand.migrated_entries, cand.claims, cand.duplicates.len(), cand.misreads), "Migration consistency check (declared formats vs state) and a dry run on a copy of the live state.", &["Whether the prior implementation can read the migrated state (recovery layer 2).", "Correctness of cumulativePaid for history predating the log."], "That the migration is consistent with the declared formats and ran cleanly in the dry run.", "The new implementation can pass every check while the old one would misread the new data.", vec![att("simulation-record", "Post-migration replay", replay_record(cand))], Concern::TechnicalValidity));
            items.push(item(&id("state-impact"), "state impact", "After migration the persistent state is format 2. Format-1 code reads slot 1 as uint64.".into(), &[Classification::Checked], "Declared by the payload's storage layout and migration spec.".into(), "Declaration comparison.", &["What format-1 code does with a packed record (see recovery)."], "The format change itself.", "None.", vec![], Concern::TechnicalValidity));
            items.push(item(
                &id("authority"),
                "authority changes",
                "No new roles or privileged functions. Upgrade admin and guardian set unchanged."
                    .into(),
                &[Classification::Checked],
                check_output(checks, "roles.unchanged"),
                "Set comparison of declared roles.",
                &[],
                "No new authority.",
                "None.",
                vec![],
                Concern::TechnicalValidity,
            ));
            items.push(item(&id("invariants"), "verified invariants", "Structural: unique selectors, interface conformance, migration consistency, writes to own slots only. Fuzzed: one payout per wallet per period on 3.0.0.".into(), &[Classification::Checked, Classification::Simulated], format!("{}. Fuzz: {}", checks_summary(checks), fuzz_line), "Deterministic checks on the parsed payload; property fuzz on the migrated model.", &["Reversibility."], "Conformance to the checked specification.", "The specification does not cover going back.", vec![att("test-output", "Fuzz report", format!("{fuzz:#?}"))], Concern::TechnicalValidity));
            items.push(item(&id("simulation"), "simulation", format!("3-period replay after migration: {} claims, {} duplicates; per-wallet totals identical to the unmigrated replay.", cand.claims, cand.duplicates.len()), &[Classification::Simulated], replay.narrative.clone(), "Replay on a migrated copy of the live state compared with a replay on the unmigrated copy.", &["Integrators' behaviour with the new records."], "What happened for the replayed claims.", "Synthetic stream.", vec![], Concern::TechnicalValidity));
            items.push(item(
                &id("economic"),
                "economic impact",
                "No change in amounts; none intended.".into(),
                &[Classification::Simulated, Classification::Judgment],
                "Per-wallet totals equal across the two replays.".into(),
                "Replay comparison.",
                &["Second-order effects through integrators."],
                "Amount neutrality in the covered cases.",
                "Whether a layout migration is worth its reversibility cost is a judgment.",
                vec![],
                Concern::Desirability,
            ));
            items.push(item(&id("recovery"), "recovery assumptions", format!("Proposer asserts: \"{}\" This package does not establish it; the recovery model checks it after execution.", artifact.manifest.migration.as_ref().map(|m| m.proposer_reversibility_claim.clone()).unwrap_or_default()), &[Classification::Assumed], "No replay in this package runs the prior implementation against migrated state.".into(), "Proposer's assertion.", &["Whether 2.3.1 can read format-2 state.", "What happens to claims made under 3.0.0 if the code is reverted."], "Nothing.", "A layout check of the prior code against its own layout would pass while it misreads the migrated data.", vec![], Concern::Both));
            items.push(item(&id("hash"), "approved payload hash", format!("H = {} (computed). Each guardian recomputes H from the tagged source before signing.", h.short()), &[Classification::Checked], format!("H = {}", h.hex()), "SHA-256 over the canonical manifest and the payload bytes.", &[], "A reproducible commitment to exactly this artifact.", "None.", vec![], Concern::TechnicalValidity));
            items.push(item(&id("decision"), "decision requested", "Approve H as written, reject it, or request changes. Recovery after migration, if ever needed, is a new proposal with its own H.".into(), &[Classification::Judgment], "Guardians decide; the package informs.".into(), "None.", &[], "Nothing; it names the decision.", "Everything above the technical layer.", vec![], Concern::Desirability));
        }
        ScenarioId::CaseD => {
            let prod = &replay.production;
            items.push(item(&id("trigger"), "trigger", format!("claimFor pays a repeated wallet once per repetition; {} duplicate payouts over {} blocks and continuing. Query over blocks {}-{} included.", prod.duplicates.len(), EXPLOIT_BLOCKS, det.block_range.0, det.block_range.1), &[Classification::Checked], det.result.clone(), "Exact query over the payout log for the exploit window.", &["Blocks after the query ran: the drain continues."], "That the duplicates occurred and how much was taken in the window.", "The loss grows every block until contained or fixed.", vec![att("query", "Trigger query", det.query.clone()), att("result", "Query result", det.rows.join("\n"))], Concern::TechnicalValidity));
            items.push(item(
                &id("incident-fix"),
                "incident fix",
                artifact.manifest.summary.clone(),
                &[Classification::Checked],
                check_output(checks, "interface.conformance"),
                "Selector-by-selector comparison; one body differs.",
                &["Other batch paths not covered by the replay."],
                "Interface unchanged; one body differs.",
                "None within the model.",
                vec![att("diff", "claimFor diff", diff.clone())],
                Concern::TechnicalValidity,
            ));
            items.push(item(
                &id("state-impact"),
                "state impact",
                "Same storage namespace, slots and types.".into(),
                &[Classification::Checked],
                check_output(checks, "storage.layout"),
                "Slot-by-slot layout comparison.",
                &[],
                "Layout compatibility.",
                "None.",
                vec![],
                Concern::TechnicalValidity,
            ));
            items.push(item(&id("authority"), "authority changes", "No new roles or privileged functions in the artifact. Whether the agent may act before quorum is a policy question outside this artifact.".into(), &[Classification::Checked, Classification::Judgment], check_output(checks, "roles.unchanged"), "Set comparison of declared roles. The incident policy is selected by the reviewer in the incident panel.", &["Any emergency power: it is not granted by this artifact."], "That the fix itself grants nothing.", "Who acts first.", vec![], Concern::Both));
            items.push(item(&id("invariants"), "verified invariants", "Structural checks pass. Fuzzed: one payout per wallet per period on 2.3.2, including batch calls.".into(), &[Classification::Checked, Classification::Simulated], format!("{}. Fuzz: {}", checks_summary(checks), fuzz_line), "Deterministic checks on the parsed payload; property fuzz.", &["Attack patterns other than in-batch repetition."], "Conformance to the checked specification.", "Other faults may exist.", vec![att("test-output", "Fuzz report", format!("{fuzz:#?}"))], Concern::TechnicalValidity));
            items.push(item(&id("simulation"), "simulation", format!("Replay of the attacker batches: 2.3.1 pays {} duplicates ({} units); 2.3.2 pays {}.", prod.duplicates.len(), prod.overpayment_gross, replay.candidate.duplicates.len()), &[Classification::Simulated], replay.narrative.clone(), "Replay on a copy of the live state with and without the candidate.", &["The attacker adapting."], "That the fix stops this pattern.", "Synthetic exploit stream.", vec![att("simulation-record", "Candidate replay", replay_record(&replay.candidate)), att("simulation-record", "Installed replay", replay_record(prod))], Concern::TechnicalValidity));
            items.push(item(&id("economic"), "economic impact", "Stops the drain. Funds already taken stay taken; recovering them is a separate decision.".into(), &[Classification::Simulated, Classification::Judgment], format!("{} units taken in the observed window.", prod.overpayment_gross), "Arithmetic on the replay.", &["Whether to pursue recovery of taken funds."], "The size of the loss so far.", "Loss continues until contained or fixed.", vec![], Concern::Desirability));
            items.push(item(&id("incident-authority"), "incident authority", "Quorum is assumed reachable at block 60 after detection. Containment options and their side effects are compared in the incident panel; none is granted by this package.".into(), &[Classification::Assumed, Classification::Judgment], "The quorum arrival time is a scenario parameter, not an observation.".into(), "None; stated as an assumption.", &["When guardians actually respond."], "Nothing.", "Where pre-authorization should stop, and who lifts a pause, is an open question.", vec![], Concern::Both));
            items.push(item(
                &id("recovery"),
                "recovery assumptions",
                "Reverting 2.3.2 to 2.3.1 reopens the fault; no state is written in a new form."
                    .into(),
                &[Classification::Checked],
                "Layout unchanged; no migration declared.".into(),
                "Layout check.",
                &[],
                "That a revert is possible and undesirable.",
                "None.",
                vec![],
                Concern::Both,
            ));
            items.push(item(&id("hash"), "approved payload hash", format!("H = {} (computed). Each guardian recomputes H from the tagged source before signing.", h.short()), &[Classification::Checked], format!("H = {}", h.hex()), "SHA-256 over the canonical manifest and the payload bytes.", &[], "A reproducible commitment to exactly this artifact.", "None.", vec![], Concern::TechnicalValidity));
            items.push(item(&id("decision"), "decision requested", "Approve H as written, reject it, or request changes. Any containment before quorum is a separate, policy-bound action; recovery of taken funds is a separate decision.".into(), &[Classification::Judgment], "Guardians decide; the package informs.".into(), "None.", &[], "Nothing; it names the decision.", "Everything above the technical layer.", vec![], Concern::Desirability));
        }
    }

    let specification_boundary: Vec<String> = match inst.id {
        ScenarioId::CaseA => vec![
            "Payout amount correctness (tier × unit) is not part of the uniqueness property.".into(),
            "The meaning of lastPaidPeriod after the change is asserted, not checked.".into(),
            "The keeper/scheduler dependency was not re-verified.".into(),
            "Behaviour outside the replayed 30 periods is not covered.".into(),
            "Whether ending the overpayment, and leaving past overpayments in place, is desirable is a guardian judgment.".into(),
        ],
        ScenarioId::CaseB => vec![
            "No check evaluates whether 2.50% is the right fee, or who should bear it.".into(),
            "User behaviour under a higher fee is not modelled.".into(),
            "Nothing here distinguishes a 1.00 → 2.50 change from a 1.00 → 25.0 change except the diff line.".into(),
        ],
        ScenarioId::CaseC => vec![
            "Whether 2.3.1 can read format-2 state is not established by this package (the recovery model shows it cannot).".into(),
            "Behaviour of integrators that index the new records is not modelled.".into(),
            "cumulativePaid for wallets whose history predates the payout log is reconstructed from the log only.".into(),
        ],
        ScenarioId::CaseD => vec![
            "Attack patterns other than in-batch repetition are not covered.".into(),
            "When the quorum actually arrives is a parameter, not an observation.".into(),
            "No emergency power is established or granted by this package.".into(),
        ],
    };
    let simulation_scope = match inst.id {
        ScenarioId::CaseA | ScenarioId::CaseB => format!("Deterministic local replay model: a synthetic stream of {} claims over {} periods (seed {:#x}) interpreted by the local protocol model. Not a chain fork; no real transactions; no bytecode.", FIXTURE_CLAIMS, FIXTURE_PERIODS, inst.stream.seed),
        ScenarioId::CaseC => "Deterministic local replay model: migration dry run on a copy of the live model state, then a synthetic 3-period stream. Not a chain fork; no real transactions; no bytecode.".into(),
        ScenarioId::CaseD => format!("Deterministic local replay model: {} synthetic attacker batches on a copy of the live model state. Not a chain fork; no real transactions; no bytecode.", EXPLOIT_BLOCKS),
    };
    let fixture_note = match inst.id {
        ScenarioId::CaseA => format!("Figures ({} wallets, 3 days, {} claims, about 0.3%) reproduce the hypothetical example in the review draft and are illustrative fixtures.", FIXTURE_AFFECTED_WALLETS, FIXTURE_CLAIMS),
        _ => "All figures are illustrative fixtures of the local model, not observations of any production system.".into(),
    };
    EvidencePackage {
        id: format!("{}-r{}", inst.proposal_id, inst.revision),
        scenario_id: inst.meta.key.clone(),
        revision: inst.revision,
        title: inst.meta.package_title.clone(),
        problem: inst.meta.problem.clone(),
        artifact: artifact.clone(),
        claimed_commitment: h,
        items,
        checks: checks.to_vec(),
        technical_verdict: verdict,
        desirability: "Left to the guardians. Not computed by the package: technical validity and desirability for the protocol are separate questions.".into(),
        specification_boundary,
        claims_complete_correctness: false,
        simulation_scope,
        decision_requested: "Approve exactly H, reject H, or request changes. A changed proposal requires a new commitment and a new approval.".into(),
        fixture_note,
    }
}

/// Produce a substituted artifact for the "reviewed is not executed" attack.
/// The agent's displayed summary is left untouched by the caller.
pub fn tamper(original: &Artifact, variant: TamperVariant) -> (Artifact, String) {
    let bundle = original.bundle().expect("original parses");
    match variant {
        TamperVariant::DivertFee => {
            let mut b = bundle;
            if let Some(c) = b.function_mut("claim(address)") {
                c.impl_id = crate::protocol::impls::CLAIM_2_3_1_DIVERTED.into();
                c.body = fixtures::CLAIM_BODY_2_3_1_DIVERTED.trim().into();
            }
            let mut a = original.clone();
            a.payload = b.payload_bytes().expect("canonical");
            (a, "Payload replaced: claim() now routes the protocol fee to a hidden address. Manifest and displayed summary unchanged.".into())
        }
        TamperVariant::RerouteSelector => {
            let mut b = bundle;
            let sel_claim = b.function("claim(address)").map(|f| f.selector.clone());
            let sel_roll = b.function("rollover()").map(|f| f.selector.clone());
            if let (Some(sc), Some(sr)) = (sel_claim, sel_roll) {
                if let Some(c) = b.function_mut("claim(address)") {
                    c.selector = sr;
                }
                if let Some(r) = b.function_mut("rollover()") {
                    r.selector = sc;
                }
            }
            let mut a = original.clone();
            a.payload = b.payload_bytes().expect("canonical");
            (a, "Payload replaced: the claim selector now routes to the privileged rollover body and vice versa. Manifest and displayed summary unchanged.".into())
        }
        TamperVariant::SilentParameter => {
            let mut b = bundle;
            b.parameters.insert("feeBps".into(), 900);
            let mut a = original.clone();
            a.payload = b.payload_bytes().expect("canonical");
            (a, "Payload replaced: feeBps set to 900 (9.00%) inside the payload. Manifest and displayed summary still show the reviewed value.".into())
        }
        TamperVariant::ManifestAndPayload => {
            let mut b = bundle;
            if let Some(c) = b.function_mut("claim(address)") {
                c.impl_id = crate::protocol::impls::CLAIM_2_3_1_DIVERTED.into();
                c.body = fixtures::CLAIM_BODY_2_3_1_DIVERTED.trim().into();
            }
            let a = Artifact::from_bundle(
                &b,
                &original.manifest.from_version,
                original.manifest.change_kind,
                &original.manifest.summary,
                &original.manifest.produced_by,
            )
            .expect("canonical");
            (a, "Manifest and payload both replaced consistently (the internal binding check passes). Displayed summary unchanged. Only H exposes the substitution.".into())
        }
    }
}
