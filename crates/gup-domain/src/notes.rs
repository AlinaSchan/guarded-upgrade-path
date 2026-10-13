//! Local review notes. Stored locally, exported as Markdown. The application
//! never presents these as supplied or approved by anyone but their author.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReviewNote {
    pub id: u64,
    pub created_seq: u64,
    pub observed_problem: String,
    pub scenario: String,
    pub technical_concern: String,
    pub suggested_policy_change: String,
    pub remaining_question: String,
    /// Optional link to one of the five questions for review (1-5).
    pub question_ref: Option<u8>,
    pub created_at: Option<String>,
}

pub fn notes_markdown(notes: &[ReviewNote]) -> String {
    let mut md = String::from("# Review notes — Guarded Upgrade Path\n\nLocal notes written by the reviewer in the desktop review aid. Nothing here is supplied or approved by the authors of the review draft.\n\n");
    if notes.is_empty() {
        md.push_str("_No notes recorded._\n");
        return md;
    }
    for n in notes {
        md.push_str(&format!(
            "## Note {}{}\n\n",
            n.id,
            n.created_at
                .as_ref()
                .map(|t| format!(" — {t}"))
                .unwrap_or_default()
        ));
        md.push_str(&format!(
            "- **Relevant scenario:** {}\n",
            if n.scenario.is_empty() {
                "—"
            } else {
                &n.scenario
            }
        ));
        if let Some(q) = n.question_ref {
            md.push_str(&format!("- **Question for review:** {q}\n"));
        }
        md.push_str(&format!(
            "\n**Observed problem**\n\n{}\n\n",
            blank(&n.observed_problem)
        ));
        md.push_str(&format!(
            "**Technical concern**\n\n{}\n\n",
            blank(&n.technical_concern)
        ));
        md.push_str(&format!(
            "**Suggested policy change**\n\n{}\n\n",
            blank(&n.suggested_policy_change)
        ));
        md.push_str(&format!(
            "**Remaining question**\n\n{}\n\n",
            blank(&n.remaining_question)
        ));
    }
    md
}

fn blank(s: &str) -> &str {
    if s.trim().is_empty() {
        "—"
    } else {
        s
    }
}
