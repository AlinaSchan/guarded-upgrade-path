//! Active incident: the authority ladder and explicitly hypothetical policies.
//!
//! A fault is being exploited now; a candidate fix exists; the quorum is not
//! yet reachable. The model compares what waiting costs and what acting
//! without quorum costs, for narrow-to-broad pre-authorized actions:
//! flag > draft a fix > limit one function > pause one package > pause everything.
//! Normal approval is the default. Any pre-authorized power is bounded, expires,
//! is scoped to one incident, and can only be lifted by guardians.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rung {
    Flag = 1,
    DraftFix = 2,
    LimitOneFunction = 3,
    PauseOnePackage = 4,
    PauseProtocol = 5,
}

impl Rung {
    pub fn all() -> [Rung; 5] {
        [
            Rung::Flag,
            Rung::DraftFix,
            Rung::LimitOneFunction,
            Rung::PauseOnePackage,
            Rung::PauseProtocol,
        ]
    }
    pub fn label(&self) -> &'static str {
        match self {
            Rung::Flag => "flag the incident",
            Rung::DraftFix => "draft a fix",
            Rung::LimitOneFunction => "limit one function",
            Rung::PauseOnePackage => "pause one package",
            Rung::PauseProtocol => "pause the entire protocol",
        }
    }
    pub fn changes_protocol_state(&self) -> bool {
        matches!(
            self,
            Rung::LimitOneFunction | Rung::PauseOnePackage | Rung::PauseProtocol
        )
    }
    pub fn side_effects(&self) -> &'static str {
        match self {
            Rung::Flag => "None on chain. Guardians are notified; nothing stops.",
            Rung::DraftFix => "None on chain. A candidate artifact exists; the exploit continues until quorum.",
            Rung::LimitOneFunction => "claimFor is disabled: integrations that batch-claim stop; single claims and exits continue.",
            Rung::PauseOnePackage => "All Distribution functions stop: no claims at all, distributions halt; exits elsewhere continue.",
            Rung::PauseProtocol => "Everything stops: users cannot exit, integrations stop, distributions halt.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentPolicy {
    pub id: String,
    pub label: String,
    /// Highest rung the agent may act on before quorum. `None` = normal approval only.
    pub pre_authorized: Option<Rung>,
    /// How long a pre-authorized state change may stand before it lapses.
    pub max_duration_blocks: u64,
    pub lift_requires_guardians: bool,
    pub assumptions: Vec<String>,
    pub is_default: bool,
    pub hypothetical: bool,
}

pub fn incident_policies() -> Vec<IncidentPolicy> {
    let mk = |id: &str,
              label: &str,
              rung: Option<Rung>,
              dur: u64,
              assumptions: &[&str],
              default: bool| IncidentPolicy {
        id: id.into(),
        label: label.into(),
        pre_authorized: rung,
        max_duration_blocks: dur,
        lift_requires_guardians: true,
        assumptions: assumptions.iter().map(|s| s.to_string()).collect(),
        is_default: default,
        hypothetical: true,
    };
    vec![
        mk("normal-approval", "Normal approval only (default)", None, 0, &["The agent observes, analyses, flags and prepares; guardians decide. No protocol state change before quorum."], true),
        mk("pre-flag", "Flag only (no protocol state change)", Some(Rung::Flag), 0, &["Flagging is ordinary agent work; nothing beyond it is pre-authorized."], false),
        mk("pre-draft", "Flag and draft only (no protocol state change)", Some(Rung::DraftFix), 0, &["Flagging and drafting are ordinary agent work; nothing beyond them is pre-authorized."], false),
        mk("pre-limit-function", "Pre-authorized: limit one function", Some(Rung::LimitOneFunction), 200, &["The agent may disable the single faulty function for at most 200 blocks.", "Only guardians can lift the limit.", "Whether the agent that drafts the fix should hold this power is an open question."], false),
        mk("pre-pause-package", "Pre-authorized: pause one package", Some(Rung::PauseOnePackage), 200, &["The agent may pause the Distribution package for at most 200 blocks.", "All claims stop, including legitimate ones.", "Only guardians can lift the pause."], false),
        mk("pre-pause-protocol", "Pre-authorized: pause everything", Some(Rung::PauseProtocol), 200, &["The agent may pause the whole protocol for at most 200 blocks.", "Users cannot exit; integrations stop; distributions halt.", "Only guardians can lift the pause."], false),
    ]
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentParams {
    pub exploit_loss_per_block: i64,
    pub legit_single_claims_per_block: u32,
    pub legit_batch_claims_per_block: u32,
    pub exits_per_block: u32,
    pub integrations_affected_by_pause: u32,
    pub agent_detection_block: u64,
    pub quorum_arrival_block: u64,
    pub horizon_blocks: u64,
    /// The diagnosis is wrong: there is no exploit. Shows the cost of premature action.
    pub false_positive: bool,
}

impl Default for IncidentParams {
    fn default() -> Self {
        IncidentParams {
            exploit_loss_per_block: 120,
            legit_single_claims_per_block: 9,
            legit_batch_claims_per_block: 4,
            exits_per_block: 2,
            integrations_affected_by_pause: 3,
            agent_detection_block: 3,
            quorum_arrival_block: 60,
            horizon_blocks: 120,
            false_positive: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Interrupted {
    pub single_claims: u64,
    pub batch_claims: u64,
    pub exits: u64,
    pub integrations: u64,
    pub distribution_blocks_halted: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub block: u64,
    pub actor: String,
    pub event: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentOutcome {
    pub policy: IncidentPolicy,
    pub false_positive: bool,
    pub loss_before_containment: i64,
    pub loss_after_containment: i64,
    pub containment_block: Option<u64>,
    pub containment_action: Option<Rung>,
    pub fix_block: u64,
    pub lifted_block: Option<u64>,
    pub interrupted: Interrupted,
    pub timeline: Vec<TimelineEntry>,
    pub authority_notes: Vec<String>,
    pub side_effects: String,
}

fn t(tl: &mut Vec<TimelineEntry>, block: u64, actor: &str, event: impl Into<String>) {
    tl.push(TimelineEntry {
        block,
        actor: actor.into(),
        event: event.into(),
    });
}

/// Simulate one incident under one policy. Deterministic.
pub fn simulate(policy: &IncidentPolicy, p: &IncidentParams) -> IncidentOutcome {
    let mut tl = Vec::new();
    let loss_rate = if p.false_positive {
        0
    } else {
        p.exploit_loss_per_block
    };
    if p.false_positive {
        t(
            &mut tl,
            0,
            "protocol",
            "No exploit is actually in progress (false positive case).",
        );
    } else {
        t(
            &mut tl,
            0,
            "attacker",
            "Exploit begins: claimFor pays a repeated wallet once per repetition.",
        );
    }
    t(
        &mut tl,
        p.agent_detection_block,
        "agent",
        "Detects the anomaly from payout monitoring; diagnoses the batch fault.",
    );
    t(
        &mut tl,
        p.agent_detection_block,
        "agent",
        "Flags the incident to guardians.",
    );
    t(
        &mut tl,
        p.agent_detection_block + 1,
        "agent",
        "Drafts candidate fix 2.3.2 and commits H.",
    );

    let mut containment_block = None;
    let mut containment_action = None;
    let mut notes = Vec::new();
    if let Some(r) = policy.pre_authorized {
        if r.changes_protocol_state() {
            let b = p.agent_detection_block + 1;
            containment_block = Some(b);
            containment_action = Some(r);
            t(&mut tl, b, "agent", format!("Acts on pre-authorized rung: {} (bounded to {} blocks; lift requires guardians).", r.label(), policy.max_duration_blocks));
            notes.push(format!("Pre-authorized action '{}' used; it stays narrower than the upgrade decision it buys time for.", r.label()));
        } else {
            notes.push(format!("Pre-authorized rung '{}' changes no protocol state; the exploit continues until quorum.", r.label()));
        }
    } else {
        notes.push("No pre-authorized action: the agent prepares; guardians decide. The exploit continues until quorum.".into());
    }

    let fix_block = p.quorum_arrival_block.max(p.agent_detection_block + 2);
    t(
        &mut tl,
        fix_block,
        "guardians",
        "Quorum reached: signatures over H for the fix.",
    );
    t(
        &mut tl,
        fix_block,
        "executor",
        "Verifies H independently and applies 2.3.2.",
    );
    let mut lifted_block = None;
    if containment_block.is_some() {
        lifted_block = Some(fix_block);
        t(
            &mut tl,
            fix_block,
            "guardians",
            "Lift the containment (guardian decision; the agent cannot lift it).",
        );
    }
    if let Some(cb) = containment_block {
        if policy.max_duration_blocks > 0 && cb + policy.max_duration_blocks < fix_block {
            t(
                &mut tl,
                cb + policy.max_duration_blocks,
                "protocol",
                "Pre-authorized containment lapses before quorum; the agent cannot renew it.",
            );
            notes.push("Containment lapsed before quorum arrived; the exploit resumed. Renewal would be a guardian decision.".into());
        }
    }

    // Losses
    let loss_until = containment_block.unwrap_or(fix_block).min(fix_block);
    let loss_before = loss_rate * loss_until as i64;
    let mut loss_after = 0i64;
    if let Some(cb) = containment_block {
        if policy.max_duration_blocks > 0 && cb + policy.max_duration_blocks < fix_block {
            loss_after = loss_rate * (fix_block - (cb + policy.max_duration_blocks)) as i64;
        }
    }

    // Interrupted legitimate activity during containment
    let mut interrupted = Interrupted::default();
    if let (Some(cb), Some(r)) = (containment_block, containment_action) {
        let span = fix_block
            .saturating_sub(cb)
            .min(if policy.max_duration_blocks > 0 {
                policy.max_duration_blocks
            } else {
                u64::MAX
            });
        match r {
            Rung::LimitOneFunction => {
                interrupted.batch_claims = p.legit_batch_claims_per_block as u64 * span;
                interrupted.integrations = p.integrations_affected_by_pause.min(1) as u64;
            }
            Rung::PauseOnePackage => {
                interrupted.batch_claims = p.legit_batch_claims_per_block as u64 * span;
                interrupted.single_claims = p.legit_single_claims_per_block as u64 * span;
                interrupted.integrations = p.integrations_affected_by_pause as u64;
                interrupted.distribution_blocks_halted = span;
            }
            Rung::PauseProtocol => {
                interrupted.batch_claims = p.legit_batch_claims_per_block as u64 * span;
                interrupted.single_claims = p.legit_single_claims_per_block as u64 * span;
                interrupted.exits = p.exits_per_block as u64 * span;
                interrupted.integrations = p.integrations_affected_by_pause as u64;
                interrupted.distribution_blocks_halted = span;
            }
            _ => {}
        }
    }
    notes.push("The agent holds no standing authority: any power here is scoped to this incident, bounded in time, and lifted only by guardians.".into());
    t(&mut tl, fix_block, "protocol", format!("Incident closed. Loss before containment {loss_before}; loss after lapse {loss_after}."));
    tl.sort_by_key(|e| e.block);
    IncidentOutcome {
        policy: policy.clone(),
        false_positive: p.false_positive,
        loss_before_containment: loss_before,
        loss_after_containment: loss_after,
        containment_block,
        containment_action,
        fix_block,
        lifted_block,
        interrupted,
        timeline: tl,
        authority_notes: notes,
        side_effects: containment_action.map(|r| r.side_effects().to_string()).unwrap_or_else(|| "No containment: the exploit runs until the fix is applied; no legitimate activity is interrupted.".into()),
    }
}

pub fn compare_policies(p: &IncidentParams) -> Vec<IncidentOutcome> {
    incident_policies()
        .iter()
        .map(|pol| simulate(pol, p))
        .collect()
}

/// A bounded emergency grant, scoped to one incident.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmergencyGrant {
    pub incident_id: String,
    pub policy_id: String,
    pub rung: Rung,
    pub granted_at_block: u64,
    pub expires_at_block: Option<u64>,
    pub used: bool,
    pub lifted_by_guardians: bool,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorityRefusal {
    #[error("no pre-authorized action under policy {0}; normal approval applies")]
    NotPreAuthorized(String),
    #[error("requested rung '{requested}' exceeds the pre-authorized rung '{allowed}'")]
    ExceedsRung { requested: String, allowed: String },
    #[error("the pre-authorized grant expired at block {0}")]
    Expired(u64),
    #[error("the grant is scoped to incident {expected}; request was for {got}")]
    WrongIncident { expected: String, got: String },
    #[error("the agent cannot lift a pause; lifting is a guardian decision")]
    AgentCannotLift,
    #[error("the incident is closed; no standing authority remains")]
    IncidentClosed,
}

/// The agent asks to act on a rung. Returns a grant only within the policy's
/// bound, for this incident, before expiry.
pub fn agent_request_action(
    policy: &IncidentPolicy,
    incident_id: &str,
    requested: Rung,
    block: u64,
    incident_open: bool,
) -> Result<EmergencyGrant, AuthorityRefusal> {
    if !incident_open {
        return Err(AuthorityRefusal::IncidentClosed);
    }
    // Flagging and drafting change no protocol state: they are the agent's ordinary
    // work (the same as Stage A) and are granted under every policy. Where exceptional,
    // pre-authorized authority begins is the open question the ladder explores.
    if !requested.changes_protocol_state() {
        return Ok(EmergencyGrant {
            incident_id: incident_id.into(),
            policy_id: policy.id.clone(),
            rung: requested,
            granted_at_block: block,
            expires_at_block: None,
            used: false,
            lifted_by_guardians: false,
        });
    }
    let allowed = policy
        .pre_authorized
        .ok_or_else(|| AuthorityRefusal::NotPreAuthorized(policy.id.clone()))?;
    if requested > allowed {
        return Err(AuthorityRefusal::ExceedsRung {
            requested: requested.label().into(),
            allowed: allowed.label().into(),
        });
    }
    Ok(EmergencyGrant {
        incident_id: incident_id.into(),
        policy_id: policy.id.clone(),
        rung: requested,
        granted_at_block: block,
        expires_at_block: if policy.max_duration_blocks > 0 && requested.changes_protocol_state() {
            Some(block + policy.max_duration_blocks)
        } else {
            None
        },
        used: false,
        lifted_by_guardians: false,
    })
}

pub fn agent_attempt_lift(_grant: &EmergencyGrant) -> Result<(), AuthorityRefusal> {
    Err(AuthorityRefusal::AgentCannotLift)
}

pub fn check_grant(
    grant: &EmergencyGrant,
    incident_id: &str,
    block: u64,
) -> Result<(), AuthorityRefusal> {
    if grant.incident_id != incident_id {
        return Err(AuthorityRefusal::WrongIncident {
            expected: grant.incident_id.clone(),
            got: incident_id.into(),
        });
    }
    if let Some(e) = grant.expires_at_block {
        if block > e {
            return Err(AuthorityRefusal::Expired(e));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waiting_costs_and_pausing_costs_are_both_visible() {
        let p = IncidentParams::default();
        let outs = compare_policies(&p);
        let normal = outs
            .iter()
            .find(|o| o.policy.id == "normal-approval")
            .unwrap();
        let limit = outs
            .iter()
            .find(|o| o.policy.id == "pre-limit-function")
            .unwrap();
        let pause_all = outs
            .iter()
            .find(|o| o.policy.id == "pre-pause-protocol")
            .unwrap();
        assert!(normal.loss_before_containment > limit.loss_before_containment);
        assert_eq!(normal.interrupted, Interrupted::default());
        assert!(limit.interrupted.batch_claims > 0 && limit.interrupted.exits == 0);
        assert!(pause_all.interrupted.exits > 0);
        assert_eq!(pause_all.lifted_block, Some(pause_all.fix_block));
    }

    #[test]
    fn false_positive_shows_cost_of_premature_action() {
        let p = IncidentParams {
            false_positive: true,
            ..Default::default()
        };
        let outs = compare_policies(&p);
        let pause = outs
            .iter()
            .find(|o| o.policy.id == "pre-pause-package")
            .unwrap();
        assert_eq!(pause.loss_before_containment, 0);
        assert!(pause.interrupted.single_claims > 0);
    }

    #[test]
    fn agent_cannot_exceed_rung_lift_or_keep_authority() {
        let pols = incident_policies();
        let normal = pols.iter().find(|p| p.is_default).unwrap();
        assert!(matches!(
            agent_request_action(normal, "inc-1", Rung::LimitOneFunction, 5, true),
            Err(AuthorityRefusal::NotPreAuthorized(_))
        ));
        assert!(agent_request_action(normal, "inc-1", Rung::Flag, 5, true).is_ok());
        assert!(agent_request_action(normal, "inc-1", Rung::DraftFix, 5, true).is_ok());
        let limit = pols.iter().find(|p| p.id == "pre-limit-function").unwrap();
        assert!(matches!(
            agent_request_action(limit, "inc-1", Rung::PauseProtocol, 5, true),
            Err(AuthorityRefusal::ExceedsRung { .. })
        ));
        let g = agent_request_action(limit, "inc-1", Rung::LimitOneFunction, 5, true).unwrap();
        assert_eq!(g.expires_at_block, Some(205));
        assert!(matches!(
            agent_attempt_lift(&g),
            Err(AuthorityRefusal::AgentCannotLift)
        ));
        assert!(matches!(
            check_grant(&g, "inc-1", 300),
            Err(AuthorityRefusal::Expired(205))
        ));
        assert!(matches!(
            check_grant(&g, "inc-2", 10),
            Err(AuthorityRefusal::WrongIncident { .. })
        ));
        assert!(matches!(
            agent_request_action(limit, "inc-1", Rung::Flag, 5, false),
            Err(AuthorityRefusal::IncidentClosed)
        ));
    }
}
