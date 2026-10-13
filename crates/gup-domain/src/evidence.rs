//! Evidence classification and the evidence package.
//!
//! Each evidence row is tagged by how it is known, so what was proven, what
//! was observed and what is left to judgment stay apart. Four categories are
//! never collapsed into a single "passed" indicator.

use crate::artifact::Artifact;
use crate::canonical::Commitment;
use crate::checks::CheckResult;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    /// Deterministic: a proof, a static check or an exact query that was actually performed.
    Checked,
    /// Seen on a replay or simulation; holds for the cases run.
    Simulated,
    /// Taken as given; nothing in the package checks it.
    Assumed,
    /// Left to the guardians. Evidence informs it, never settles it.
    Judgment,
}

impl Classification {
    pub fn label(&self) -> &'static str {
        match self {
            Classification::Checked => "checked",
            Classification::Simulated => "simulated",
            Classification::Assumed => "assumed",
            Classification::Judgment => "judgment",
        }
    }
    pub fn definition(&self) -> &'static str {
        match self {
            Classification::Checked => "Deterministic: a proof, a static check or an exact query.",
            Classification::Simulated => "Seen on a fork or replay; holds for the cases run.",
            Classification::Assumed => "Taken as given; nothing in the package checks it.",
            Classification::Judgment => {
                "Left to the guardians. Evidence informs it, never settles it."
            }
        }
    }
}

/// A supporting artifact attached to an evidence item (query text, diff,
/// test output, simulation record).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub kind: String,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub id: String,
    /// Row label as in the review draft (trigger, change surface, ...).
    pub row: String,
    pub claim: String,
    pub classifications: Vec<Classification>,
    pub evidence: String,
    pub method: String,
    pub not_checked: Vec<String>,
    pub establishes: String,
    pub remaining_uncertainty: String,
    pub attachments: Vec<Attachment>,
    /// Which evidence this row speaks to: technical validity or protocol desirability.
    pub concerns: Concern,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Concern {
    TechnicalValidity,
    Desirability,
    Both,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TechnicalVerdict {
    pub checked_passed: Vec<String>,
    pub checked_failed: Vec<String>,
    pub simulated: Vec<String>,
    pub all_checked_passed: bool,
    pub statement: String,
}

/// The package guardians review. The commitment is computed from
/// `artifact`; `claimed_commitment` is what the agent displays. The two are
/// kept separate on purpose so that an independent recomputation means
/// something.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePackage {
    pub id: String,
    pub scenario_id: String,
    pub revision: u32,
    pub title: String,
    pub problem: String,
    pub artifact: Artifact,
    pub claimed_commitment: Commitment,
    pub items: Vec<EvidenceItem>,
    pub checks: Vec<CheckResult>,
    pub technical_verdict: TechnicalVerdict,
    pub desirability: String,
    /// What the checks' specification does not cover. Always non-empty.
    pub specification_boundary: Vec<String>,
    pub claims_complete_correctness: bool,
    pub simulation_scope: String,
    pub decision_requested: String,
    pub fixture_note: String,
}

impl EvidencePackage {
    pub fn items_with(&self, c: Classification) -> Vec<&EvidenceItem> {
        self.items
            .iter()
            .filter(|i| i.classifications.contains(&c))
            .collect()
    }

    pub fn assumptions(&self) -> Vec<&EvidenceItem> {
        self.items_with(Classification::Assumed)
    }

    pub fn judgments(&self) -> Vec<&EvidenceItem> {
        self.items_with(Classification::Judgment)
    }

    pub fn counts(&self) -> [usize; 4] {
        [
            self.items_with(Classification::Checked).len(),
            self.items_with(Classification::Simulated).len(),
            self.items_with(Classification::Assumed).len(),
            self.items_with(Classification::Judgment).len(),
        ]
    }

    /// Recompute `H` from the artifact as held. Does not consult `claimed_commitment`.
    pub fn recompute_commitment(&self) -> Commitment {
        self.artifact
            .commitment()
            .expect("manifest is canonicalizable")
    }
}

pub fn verdict_from_checks(checks: &[CheckResult], simulated: &[String]) -> TechnicalVerdict {
    let passed: Vec<String> = checks
        .iter()
        .filter(|c| c.passed)
        .map(|c| c.id.clone())
        .collect();
    let failed: Vec<String> = checks
        .iter()
        .filter(|c| !c.passed)
        .map(|c| c.id.clone())
        .collect();
    let all = failed.is_empty();
    TechnicalVerdict {
        statement: if all {
            format!(
                "{} deterministic checks passed and {} simulated properties held for the cases run. This establishes conformance to the checked specification, not that the change is correct or desirable.",
                passed.len(),
                simulated.len()
            )
        } else {
            format!(
                "{} of {} deterministic checks failed: {:?}.",
                failed.len(),
                checks.len(),
                failed
            )
        },
        checked_passed: passed,
        checked_failed: failed,
        simulated: simulated.to_vec(),
        all_checked_passed: all,
    }
}
