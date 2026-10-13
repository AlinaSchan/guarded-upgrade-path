//! The five questions for review, verbatim from page 4 of the review draft.
//! They are unresolved and stay unresolved unless an actual reviewer answers
//! them; the application never fills in an answer.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionLink {
    pub label: String,
    /// Frontend route: overview, scenario, approval, execution, recovery, incident, failures, lab
    pub route: String,
    pub scenario: Option<String>,
    pub failure_mode: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewQuestion {
    pub number: u8,
    pub text: String,
    pub concerns: String,
    pub links: Vec<QuestionLink>,
    pub status: String,
}

fn link(label: &str, route: &str, scenario: Option<&str>, failure: Option<&str>) -> QuestionLink {
    QuestionLink {
        label: label.into(),
        route: route.into(),
        scenario: scenario.map(|s| s.to_string()),
        failure_mode: failure.map(|s| s.to_string()),
    }
}

pub fn questions() -> Vec<ReviewQuestion> {
    vec![
        ReviewQuestion {
            number: 1,
            text: "If a package is proven structurally sound (selectors, storage layout, interface conformance), what would you still need to see before voting to deploy it?".into(),
            concerns: "What evidence is necessary beyond structural soundness.".into(),
            links: vec![
                link("Case A evidence package: checked vs assumed rows", "scenario", Some("A"), None),
                link("Failure 1: the proof holds, the property was wrong", "failures", Some("A"), Some("F1")),
                link("Independent verification of H", "execution", Some("A"), None),
            ],
            status: "unresolved".into(),
        },
        ReviewQuestion {
            number: 2,
            text: "When a guardian is asked to sign under time pressure, would a structured evidence package improve the decision, or does a meaningful signature still require reading the implementation for most upgrades?".into(),
            concerns: "Whether meaningful guardian approval requires reading implementation code.".into(),
            links: vec![
                link("Approval surface comparison (modes A, B, C)", "approval", Some("A"), None),
                link("Failure 2: checks pass, judgment stops", "failures", Some("A"), Some("F2")),
            ],
            status: "unresolved".into(),
        },
        ReviewQuestion {
            number: 3,
            text: "Where does rollback stop being the right concept once an upgrade has changed persistent state, and what should a proposal commit to about recovery before it is approved?".into(),
            concerns: "Where rollback stops being sufficient.".into(),
            links: vec![
                link("Case C: state migration and the four-layer recovery assessment", "recovery", Some("C"), None),
                link("Failure 4: a rollback that isn't", "failures", Some("C"), Some("F4")),
            ],
            status: "unresolved".into(),
        },
        ReviewQuestion {
            number: 4,
            text: "Should a parameter-only change, such as a fee or a distribution split, ever take a lighter path than an implementation change when its economic effect can be larger?".into(),
            concerns: "Whether economically significant parameter changes deserve lighter approval.".into(),
            links: vec![
                link("Case B: economic effect of feeBps 100 → 250", "scenario", Some("B"), None),
                link("Quorum policies, including the hypothetical lighter path", "approval", Some("B"), None),
                link("Failure 3: valid code, unwanted economics", "failures", Some("B"), Some("F3")),
            ],
            status: "unresolved".into(),
        },
        ReviewQuestion {
            number: 5,
            text: "During an active incident, what is the narrowest action you would pre-authorize before quorum, and should the agent that drafts the fix ever be the one holding it?".into(),
            concerns: "Which emergency actions could be pre-authorized before quorum.".into(),
            links: vec![
                link("Case D: incident authority ladder and policy comparison", "incident", Some("D"), None),
                link("Failure 6: quorum arrives late, or authority creeps", "failures", Some("D"), Some("F6")),
            ],
            status: "unresolved".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_questions_each_linked_and_unresolved() {
        let qs = questions();
        assert_eq!(qs.len(), 5);
        for (i, q) in qs.iter().enumerate() {
            assert_eq!(q.number as usize, i + 1);
            assert!(!q.links.is_empty());
            assert_eq!(q.status, "unresolved");
        }
        assert!(qs[0]
            .text
            .starts_with("If a package is proven structurally sound"));
        assert!(qs[4].text.ends_with("the one holding it?"));
    }
}
