//! Markdown exports of the reviewer's observations.

use crate::engine::{ExecutionOutcome, Workspace};
use crate::notes::notes_markdown;

pub fn observations_markdown(ws: &Workspace) -> String {
    let mut md = String::from("# Review observations — Guarded Upgrade Path\n\nState of the review sandbox at export time. Every figure comes from the deterministic local model; nothing here is an observation of a production system, and nothing here is an answer supplied by the external reviewer.\n\n");
    for (key, inst) in &ws.scenarios {
        md.push_str(&format!("## Case {} — {}\n\n", key, inst.meta.title));
        md.push_str(&format!(
            "- Package: {}\n- Proposal: {} (revision {})\n- Status: {} ({} stage)\n",
            inst.meta.package_title,
            inst.proposal_id,
            inst.revision,
            inst.status.label(),
            inst.status.stage()
        ));
        if let Some(h) = inst.current_commitment() {
            md.push_str(&format!("- Artifact as held: H = {}\n", h.hex()));
        }
        if let Some(s) = &inst.displayed_summary {
            md.push_str(&format!(
                "- Agent-displayed H: {}\n",
                s.claimed_commitment.hex()
            ));
        }
        if let Some(t) = &inst.tamper {
            md.push_str(&format!(
                "- Substitution in effect: {:?} — {}\n",
                t.variant, t.description
            ));
        }
        if let Some(p) = &inst.package {
            let [c, s, a, j] = p.counts();
            md.push_str(&format!("- Evidence rows: {} ({c} checked, {s} simulated, {a} assumed, {j} judgment)\n- Technical verdict: {}\n- Desirability: {}\n", p.items.len(), p.technical_verdict.statement, p.desirability));
            md.push_str("- Specification boundary:\n");
            for b in &p.specification_boundary {
                md.push_str(&format!("  - {b}\n"));
            }
        }
        md.push_str(&format!(
            "- Quorum policy: {} ({}-of-{})\n",
            inst.quorum_policy.label, inst.quorum_policy.m, inst.quorum_policy.n
        ));
        for ap in &inst.approvals {
            md.push_str(&format!(
                "- {}: {:?} over {} (viewed surface {:?})\n",
                ap.guardian_id,
                ap.decision,
                ap.commitment.short(),
                ap.surface_mode_viewed
            ));
        }
        match &inst.execution {
            Some(ExecutionOutcome::Applied { report }) => {
                md.push_str(&format!("- Execution: {}\n", report.statement))
            }
            Some(ExecutionOutcome::Refused { refusal }) => {
                md.push_str(&format!("- Execution: {}\n", refusal.summary()))
            }
            None => {}
        }
        if let Some(r) = &inst.recovery {
            md.push_str(&format!("- Recovery: {}\n", r.statement));
            for l in &r.layers {
                md.push_str(&format!(
                    "  - Layer {} {}: {:?} — {}\n",
                    l.layer, l.title, l.status, l.finding
                ));
            }
        }
        if let Some(inc) = &inst.incident {
            md.push_str(&format!(
                "- Incident {} ({}): policy {}\n",
                inc.incident_id,
                if inc.open { "open" } else { "closed" },
                inc.policy_id
            ));
            for o in &inc.outcomes {
                md.push_str(&format!("  - {}: loss {} before containment; interrupted {} single / {} batch claims, {} exits\n", o.policy.label, o.loss_before_containment, o.interrupted.single_claims, o.interrupted.batch_claims, o.interrupted.exits));
            }
        }
        if !inst.narrative.is_empty() {
            md.push_str("\nNarrative:\n\n");
            for line in &inst.narrative {
                md.push_str(&format!("- {line}\n"));
            }
        }
        md.push('\n');
    }
    if !ws.failure_results.is_empty() {
        md.push_str("## Failure modes run\n\n");
        for (k, r) in &ws.failure_results {
            md.push_str(&format!(
                "- {} {}: expected behaviour {} — {}\n",
                k,
                r.title,
                if r.holds { "observed" } else { "NOT observed" },
                r.observed
            ));
        }
        md.push('\n');
    }
    md.push_str("## Audit events\n\n");
    for e in ws.audit.iter().rev().take(60) {
        md.push_str(&format!(
            "- #{} [{}] {} — {}: {}{}\n",
            e.seq,
            e.scenario.clone().unwrap_or_else(|| "—".into()),
            e.actor,
            e.action,
            e.detail,
            e.commitment
                .as_ref()
                .map(|c| format!(" ({c})"))
                .unwrap_or_default()
        ));
    }
    md.push('\n');
    md.push_str(&notes_markdown(&ws.notes));
    md
}
