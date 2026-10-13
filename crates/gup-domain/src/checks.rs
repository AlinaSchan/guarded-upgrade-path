//! Deterministic structural checks.
//!
//! Every check runs against the **parsed payload** (the bundle the executor
//! would install), never against the manifest's or the agent's description of
//! it. Each result states its method, its scope and what it does not cover,
//! so a passing check can never be read as "the change is correct".

use crate::artifact::{ArtifactManifest, ImplementationBundle, RoleSet};
use crate::canonical::sha256_hex;
use crate::evidence::Classification;
use crate::protocol::PackageState;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub name: String,
    pub passed: bool,
    pub classification: Classification,
    pub method: String,
    pub scope: String,
    pub output: String,
    pub not_covered: Vec<String>,
}

impl CheckResult {
    fn new(
        id: &str,
        name: &str,
        passed: bool,
        method: &str,
        scope: &str,
        output: String,
        not_covered: &[&str],
    ) -> Self {
        CheckResult {
            id: id.into(),
            name: name.into(),
            passed,
            classification: Classification::Checked,
            method: method.into(),
            scope: scope.into(),
            output,
            not_covered: not_covered.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Run the structural checks of a candidate bundle against the currently
/// installed package and roles.
pub fn structural_checks(
    current: &PackageState,
    current_roles: &RoleSet,
    current_format_version: u32,
    candidate: &ImplementationBundle,
) -> Vec<CheckResult> {
    let mut out = Vec::new();

    // 1. Unique selectors
    {
        let mut seen = BTreeSet::new();
        let mut dups = Vec::new();
        for f in &candidate.functions {
            if !seen.insert(f.selector.clone()) {
                dups.push(f.selector.clone());
            }
        }
        out.push(CheckResult::new(
            "selectors.unique",
            "Unique selectors",
            dups.is_empty(),
            "Exact comparison of the selector of every function carried in the payload.",
            "All functions in the candidate bundle.",
            if dups.is_empty() {
                format!("{} selectors, no collisions.", seen.len())
            } else {
                format!("Colliding selectors: {dups:?}")
            },
            &["Whether a selector routes to the function its name suggests."],
        ));
    }

    // 2. Interface conformance: same selector set, same names, no rerouting
    {
        let cur: BTreeMap<&str, &str> = current
            .functions
            .values()
            .map(|f| (f.selector.as_str(), f.name.as_str()))
            .collect();
        let cand: BTreeMap<&str, &str> = candidate
            .functions
            .iter()
            .map(|f| (f.selector.as_str(), f.name.as_str()))
            .collect();
        let added: Vec<&str> = cand
            .keys()
            .filter(|k| !cur.contains_key(*k))
            .copied()
            .collect();
        let removed: Vec<&str> = cur
            .keys()
            .filter(|k| !cand.contains_key(*k))
            .copied()
            .collect();
        let rerouted: Vec<String> = cand
            .iter()
            .filter_map(|(sel, name)| {
                cur.get(sel)
                    .filter(|n| n != &name)
                    .map(|n| format!("{sel}: {n} -> {name}"))
            })
            .collect();
        let changed_bodies: Vec<String> = candidate
            .functions
            .iter()
            .filter(|f| {
                current
                    .installed
                    .function(&f.name)
                    .map(|c| c.body != f.body || c.impl_id != f.impl_id)
                    .unwrap_or(false)
            })
            .map(|f| f.name.clone())
            .collect();
        let passed = added.is_empty() && removed.is_empty() && rerouted.is_empty();
        out.push(CheckResult::new(
            "interface.conformance",
            "Interface conformance",
            passed,
            "Selector-by-selector comparison of the installed interface with the payload's declared functions.",
            "Function selectors and names. Bodies are compared only to report which changed.",
            format!(
                "added: {added:?}; removed: {removed:?}; rerouted: {rerouted:?}; function bodies changed: {changed_bodies:?}"
            ),
            &["What the changed function bodies do.", "Whether callers depend on the old behaviour."],
        ));
    }

    // 3. Storage layout: grow-only, typed, same namespace (or consistent migration)
    {
        let cur = &current.storage;
        let cand = &candidate.storage;
        let mut problems = Vec::new();
        if cur.namespace != cand.namespace {
            problems.push(format!("namespace {} -> {}", cur.namespace, cand.namespace));
        }
        let (passed, method, output, not_covered): (bool, String, String, Vec<&str>) = if let Some(
            m,
        ) =
            &candidate.migration
        {
            // With a declared migration the check verifies the migration is
            // consistent with the formats involved, not that the old code can
            // still read the state.
            let consistent = m.from_format == current_format_version
                && m.to_format == cand.format_version
                && m.from_format != m.to_format;
            if !consistent {
                problems.push(format!(
                    "migration {} declares {} -> {} but state is format {} and layout is format {}",
                    m.id, m.from_format, m.to_format, current_format_version, cand.format_version
                ));
            }
            (
                problems.is_empty(),
                "Migration consistency: declared from/to formats match the current state format and the candidate layout's format.".to_string(),
                format!("migration {}: format {} -> {}; {} slots after migration; problems: {problems:?}", m.id, m.from_format, m.to_format, cand.slots.len()),
                vec!["Whether the prior implementation can read the migrated state (recovery layer 2).", "Whether the transform preserves every value's meaning."],
            )
        } else {
            for s in &cur.slots {
                match cand.slot(&s.name) {
                    None => problems.push(format!("slot {} ({}) removed", s.index, s.name)),
                    Some(c) if c.ty != s.ty || c.index != s.index => problems.push(format!(
                        "slot {} ({}) changed: {}@{} -> {}@{}",
                        s.index, s.name, s.ty, s.index, c.ty, c.index
                    )),
                    _ => {}
                }
            }
            let max_cur = cur.slots.iter().map(|s| s.index).max().unwrap_or(0);
            for c in &cand.slots {
                if cur.slot(&c.name).is_none() && c.index <= max_cur {
                    problems.push(format!(
                        "new slot {} ({}) inserted below existing slots",
                        c.index, c.name
                    ));
                }
            }
            if cur.format_version != cand.format_version {
                problems.push(format!(
                    "format version {} -> {} without a declared migration",
                    cur.format_version, cand.format_version
                ));
            }
            (
                problems.is_empty(),
                "Slot-by-slot comparison of namespace, index and declared type; new slots must append.".to_string(),
                format!("namespace {}; {} existing slots compared; {} candidate slots; problems: {problems:?}", cur.namespace, cur.slots.len(), cand.slots.len()),
                vec!["Whether a slot keeps its meaning (e.g. what lastPaidPeriod denotes).", "Values written at runtime."],
            )
        };
        out.push(CheckResult::new(
            "storage.layout",
            "Storage layout (grow-only, typed)",
            passed,
            &method,
            "Declared storage layout of the candidate vs the installed package.",
            output,
            &not_covered,
        ));
    }

    // 4. Writes to own slots only
    {
        let own: BTreeSet<String> = candidate.storage.slot_names().into_iter().collect();
        let mut foreign = Vec::new();
        for f in &candidate.functions {
            for w in &f.writes {
                if !own.contains(w) {
                    foreign.push(format!("{} writes {}", f.name, w));
                }
            }
        }
        out.push(CheckResult::new(
            "storage.writes_own_slots",
            "Writes to own slots only",
            foreign.is_empty(),
            "Declared write set of every function compared with the package's own namespace.",
            "Declared writes in the payload (static declaration, not a trace of execution).",
            if foreign.is_empty() {
                format!(
                    "{} functions, all writes inside {}.*",
                    candidate.functions.len(),
                    candidate.storage.namespace
                )
            } else {
                format!("foreign writes: {foreign:?}")
            },
            &[
                "Writes performed through external calls.",
                "Reads of foreign state.",
            ],
        ));
    }

    // 5. Roles unchanged
    {
        let cur = current_roles.clone().normalized();
        let cand = candidate.roles.clone().normalized();
        let passed = cur == cand;
        out.push(CheckResult::new(
            "roles.unchanged",
            "Upgrade admin and guardian set unchanged",
            passed,
            "Exact set comparison of the declared upgrade-admin and guardian sets.",
            "Role declarations carried in the payload.",
            if passed {
                format!(
                    "upgrade admin {:?}; guardians {:?}; unchanged.",
                    cur.upgrade_admin, cur.guardians
                )
            } else {
                format!("current {cur:?} vs candidate {cand:?}")
            },
            &["Off-chain control of the keys behind these roles."],
        ));
    }

    // 6. No new privileged functions
    {
        let cur_priv: BTreeSet<&str> = current
            .functions
            .values()
            .filter(|f| f.privileged)
            .map(|f| f.name.as_str())
            .collect();
        let new_priv: Vec<&str> = candidate
            .functions
            .iter()
            .filter(|f| f.privileged && !cur_priv.contains(f.name.as_str()))
            .map(|f| f.name.as_str())
            .collect();
        out.push(CheckResult::new(
            "functions.no_new_privileged",
            "No new privileged functions",
            new_priv.is_empty(),
            "Comparison of the privileged flag of every function with the installed package.",
            "Declared privilege flags.",
            if new_priv.is_empty() {
                format!("{} privileged functions before and after.", cur_priv.len())
            } else {
                format!("new privileged functions: {new_priv:?}")
            },
            &["Whether an unprivileged function gained privileged effect through its body."],
        ));
    }

    // 7. Parameter diff (informational)
    {
        let mut diffs = Vec::new();
        for (k, v) in &candidate.parameters {
            match current.parameters.get(k) {
                Some(c) if c == v => {}
                Some(c) => diffs.push(format!("{k}: {c} -> {v}")),
                None => diffs.push(format!("{k}: (new) {v}")),
            }
        }
        for k in current.parameters.keys() {
            if !candidate.parameters.contains_key(k) {
                diffs.push(format!("{k}: removed"));
            }
        }
        out.push(CheckResult::new(
            "parameters.diff",
            "Parameter changes",
            true,
            "Exact value comparison of every declared parameter.",
            "Declared parameters.",
            if diffs.is_empty() {
                "No parameter changes.".into()
            } else {
                format!("changed: {diffs:?}")
            },
            &["Whether a parameter value is desirable (an economic judgment)."],
        ));
    }

    out
}

/// Verify that a manifest describes the payload it is attached to.
pub fn manifest_binding_check(manifest: &ArtifactManifest, payload: &[u8]) -> CheckResult {
    let mut problems = Vec::new();
    let actual = sha256_hex(payload);
    if manifest.payload_sha256 != actual {
        problems.push(format!(
            "payload_sha256 {} != sha256(payload) {}",
            manifest.payload_sha256, actual
        ));
    }
    match ImplementationBundle::from_payload_bytes(payload) {
        Err(e) => problems.push(format!("payload unparseable: {e}")),
        Ok(b) => {
            if b.entries() != manifest.functions {
                problems.push("declared functions differ from the payload's functions".into());
            }
            if b.storage != manifest.storage {
                problems.push("declared storage layout differs from the payload's".into());
            }
            if b.parameters != manifest.parameters {
                problems.push("declared parameters differ from the payload's".into());
            }
            if b.roles.clone().normalized() != manifest.roles.clone().normalized() {
                problems.push("declared roles differ from the payload's".into());
            }
            if b.migration != manifest.migration {
                problems.push("declared migration differs from the payload's".into());
            }
            if b.version != manifest.to_version || b.package != manifest.package {
                problems.push("package or version differs".into());
            }
        }
    }
    CheckResult::new(
        "payload.binds_manifest",
        "Manifest describes this payload",
        problems.is_empty(),
        "SHA-256 of the payload compared with the manifest, and every manifest field compared with the parsed payload.",
        "Manifest vs payload bytes as held right now.",
        if problems.is_empty() { format!("payload sha256 {actual}; functions, storage, parameters, roles and migration agree.") } else { format!("mismatch: {problems:?}") },
        &["Whether either describes what guardians believe they approved; that is what H and the verifier establish."],
    )
}

pub fn all_passed(checks: &[CheckResult]) -> bool {
    checks.iter().all(|c| c.passed)
}
