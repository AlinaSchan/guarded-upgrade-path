//! Approval surface projections.
//!
//! The same evidence package can be presented three ways. The projection is
//! pure: switching modes never touches the package or its commitment. Mode A
//! must say what it leaves out. No mode is presented as the correct one; the
//! review draft calls the right size of the surface an open question.

use crate::canonical::Commitment;
use crate::evidence::{Attachment, Classification, EvidencePackage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceMode {
    Minimal,
    Structured,
    Full,
}

impl SurfaceMode {
    pub fn label(&self) -> &'static str {
        match self {
            SurfaceMode::Minimal => "A. Minimal",
            SurfaceMode::Structured => "B. Structured",
            SurfaceMode::Full => "C. Full technical evidence",
        }
    }
    pub fn description(&self) -> &'static str {
        match self {
            SurfaceMode::Minimal => "\"All automated checks passed.\" One line. The guardian trusts whoever chose the checks.",
            SurfaceMode::Structured => "What changes in code, state, authority and value; what was not checked; what can be undone.",
            SurfaceMode::Full => "Full diff, traces, test output, proofs and simulation data. Complete, and unread under time pressure.",
        }
    }
    pub fn all() -> [SurfaceMode; 3] {
        [
            SurfaceMode::Minimal,
            SurfaceMode::Structured,
            SurfaceMode::Full,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSection {
    pub title: String,
    pub lines: Vec<String>,
    pub item_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceView {
    pub mode: SurfaceMode,
    pub label: String,
    pub description: String,
    pub commitment: Commitment,
    pub headline: String,
    pub sections: Vec<SurfaceSection>,
    /// What this presentation leaves out, stated explicitly.
    pub omitted: Vec<String>,
    pub attachments: Vec<Attachment>,
    pub items_shown: usize,
    pub items_total: usize,
    pub note: String,
}

fn section(title: &str, lines: Vec<String>, item_ids: Vec<String>) -> SurfaceSection {
    SurfaceSection {
        title: title.into(),
        lines,
        item_ids,
    }
}

fn rows<'a>(pkg: &'a EvidencePackage, names: &[&str]) -> Vec<&'a crate::evidence::EvidenceItem> {
    pkg.items
        .iter()
        .filter(|i| names.contains(&i.row.as_str()))
        .collect()
}

fn ids(items: &[&crate::evidence::EvidenceItem]) -> Vec<String> {
    items.iter().map(|i| i.id.clone()).collect()
}

fn lines(items: &[&crate::evidence::EvidenceItem]) -> Vec<String> {
    items
        .iter()
        .map(|i| {
            format!(
                "{} [{}]",
                i.claim,
                i.classifications
                    .iter()
                    .map(|c| c.label())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect()
}

/// Project a package into a presentation. The commitment reported is
/// recomputed from the artifact, never copied from the agent's claim.
pub fn project(pkg: &EvidencePackage, mode: SurfaceMode) -> SurfaceView {
    let commitment = pkg.recompute_commitment();
    let [checked, simulated, assumed, judgment] = pkg.counts();
    let total = pkg.items.len();
    match mode {
        SurfaceMode::Minimal => {
            let headline = if pkg.technical_verdict.all_checked_passed {
                "All automated checks passed.".to_string()
            } else {
                format!(
                    "{} automated checks failed.",
                    pkg.technical_verdict.checked_failed.len()
                )
            };
            let mut omitted = vec![
                "Which properties were checked and which were not.".to_string(),
                format!(
                    "{assumed} assumption rows: {}",
                    pkg.assumptions()
                        .iter()
                        .map(|i| i.row.clone())
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                format!(
                    "{judgment} judgment rows: {}",
                    pkg.judgments()
                        .iter()
                        .map(|i| i.row.clone())
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                "The specification boundary (what the checks do not establish).".to_string(),
                "Economic effect and who bears it.".to_string(),
                "Reversibility and recovery limitations.".to_string(),
                "Simulation scope (what the replay covered).".to_string(),
            ];
            omitted.retain(|o| !o.ends_with(": "));
            SurfaceView {
                mode,
                label: mode.label().into(),
                description: mode.description().into(),
                commitment,
                headline,
                sections: vec![section(
                    "Summary",
                    vec![format!("{checked} checked, {simulated} simulated. Artifact {}.", commitment.short())],
                    vec![],
                )],
                omitted,
                attachments: vec![],
                items_shown: 0,
                items_total: total,
                note: "This surface shows the result of the checks, not their specification. Whoever chose the checks chose what this line can say.".into(),
            }
        }
        SurfaceMode::Structured | SurfaceMode::Full => {
            let code = rows(pkg, &["change surface", "parameter change", "incident fix"]);
            let state = rows(pkg, &["state impact", "migration"]);
            let authority = rows(pkg, &["authority changes", "incident authority"]);
            let economic = rows(pkg, &["economic impact"]);
            let verification = rows(pkg, &["trigger", "verified invariants"]);
            let assumptions: Vec<_> = pkg
                .items
                .iter()
                .filter(|i| i.classifications.contains(&Classification::Assumed))
                .collect();
            let simulation = rows(pkg, &["simulation"]);
            let reversibility = rows(pkg, &["recovery assumptions"]);
            let decision = rows(pkg, &["decision requested"]);

            let mut verification_lines = lines(&verification);
            for c in &pkg.checks {
                verification_lines.push(format!(
                    "{} — {} — {}",
                    c.name,
                    if c.passed { "passed" } else { "FAILED" },
                    c.output
                ));
            }
            let mut recovery_lines: Vec<String> = pkg
                .items
                .iter()
                .flat_map(|i| {
                    i.not_checked
                        .iter()
                        .map(move |n| format!("{}: not checked — {n}", i.row))
                })
                .collect();
            recovery_lines.push("Recovery after an unexpected post-state is a new guardian decision, not a rollback.".into());
            let mut sections = vec![
                section("Code changes", lines(&code), ids(&code)),
                section("State changes", lines(&state), ids(&state)),
                section("Authority changes", lines(&authority), ids(&authority)),
                section("Economic effects", lines(&economic), ids(&economic)),
                section("Verification results", verification_lines, ids(&verification)),
                section("Assumptions", lines(&assumptions), ids(&assumptions)),
                section("Simulation scope", { let mut l = lines(&simulation); l.push(pkg.simulation_scope.clone()); l }, ids(&simulation)),
                section("Reversibility", lines(&reversibility), ids(&reversibility)),
                section("Recovery limitations", recovery_lines, vec![]),
                section(
                    "Exact artifact commitment",
                    vec![
                        format!("H = {}", commitment.hex()),
                        "Recomputed from the manifest and payload as held; see docs/VERIFICATION.md to reproduce with gup-verify or sha256sum.".into(),
                        format!("Agent-displayed value: {} ({})", pkg.claimed_commitment.hex(), if pkg.claimed_commitment == commitment { "matches" } else { "DOES NOT MATCH the recomputed value" }),
                    ],
                    vec![],
                ),
                section("Decision requested", lines(&decision), ids(&decision)),
            ];
            let mut attachments = Vec::new();
            if mode == SurfaceMode::Full {
                for i in &pkg.items {
                    attachments.extend(i.attachments.iter().cloned());
                }
                attachments.push(Attachment {
                    kind: "manifest".into(),
                    title: "Artifact manifest (JSON)".into(),
                    body: pkg.artifact.manifest_pretty(),
                });
                attachments.push(Attachment {
                    kind: "payload".into(),
                    title: "Payload (implementation bundle, canonical JSON)".into(),
                    body: pkg.artifact.payload_utf8(),
                });
                for c in &pkg.checks {
                    attachments.push(Attachment {
                        kind: "check".into(),
                        title: format!("Check {}: {}", c.id, c.name),
                        body: format!(
                            "passed: {}\nmethod: {}\nscope: {}\noutput: {}\nnot covered: {}",
                            c.passed,
                            c.method,
                            c.scope,
                            c.output,
                            c.not_covered.join("; ")
                        ),
                    });
                }
                sections.push(section(
                    "Specification boundary",
                    pkg.specification_boundary.clone(),
                    vec![],
                ));
            }
            let shown = code.len()
                + state.len()
                + authority.len()
                + economic.len()
                + verification.len()
                + assumptions.len()
                + simulation.len()
                + reversibility.len()
                + decision.len();
            SurfaceView {
                mode,
                label: mode.label().into(),
                description: mode.description().into(),
                commitment,
                headline: pkg.technical_verdict.statement.clone(),
                sections,
                omitted: if mode == SurfaceMode::Full {
                    vec!["Nothing withheld from the package. Whether a guardian reads all of it under time pressure is the open question.".into()]
                } else {
                    vec!["Raw diffs, query text, full test output and simulation records (available in mode C).".into()]
                },
                attachments,
                items_shown: shown.min(total),
                items_total: total,
                note: "Technically valid and desirable for the protocol are separate questions; the rows tagged judgment are not settled by any check above.".into(),
            }
        }
    }
}

/// Side-by-side comparison for the reviewer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceComparison {
    pub commitment_identical: bool,
    pub package_unchanged: bool,
    pub views: Vec<SurfaceView>,
    pub working_guess: String,
    pub open_question: String,
}

pub fn compare(pkg: &EvidencePackage) -> SurfaceComparison {
    let before = pkg.clone();
    let views: Vec<SurfaceView> = SurfaceMode::all()
        .iter()
        .map(|m| project(pkg, *m))
        .collect();
    let commitments: Vec<Commitment> = views.iter().map(|v| v.commitment).collect();
    SurfaceComparison {
        commitment_identical: commitments.iter().all(|c| *c == commitments[0]),
        package_unchanged: before == *pkg,
        views,
        working_guess: "Working guess: the surface grows with changes to state and authority, not with lines of code.".into(),
        open_question: "How much evidence is enough to make the decision meaningful without turning every vote into a full audit?".into(),
    }
}
