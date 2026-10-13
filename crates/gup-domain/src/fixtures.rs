//! Deterministic example artifacts.
//!
//! These bundles stand in for compiled packages. Their bodies are readable
//! pseudo-source; their behaviour is interpreted by the local protocol model
//! through `impl_id`. They are illustrative fixtures, not audited contracts.

use crate::artifact::{
    selector_for, FunctionImpl, ImplementationBundle, MigrationSpec, RoleSet, StorageLayout,
    StorageSlot, BUNDLE_FORMAT,
};
use crate::protocol::{impls, DISTRIBUTION};
use std::collections::BTreeMap;

pub const UPGRADE_ADMIN: &str = "upgrade-admin";
pub const GUARDIAN_IDS: [&str; 3] = ["guardian-1", "guardian-2", "guardian-3"];
pub const DEFAULT_FEE_BPS: i64 = 100;
pub const CASE_B_FEE_BPS: i64 = 250;

pub fn roles() -> RoleSet {
    RoleSet {
        upgrade_admin: vec![UPGRADE_ADMIN.to_string()],
        guardians: GUARDIAN_IDS.iter().map(|s| s.to_string()).collect(),
    }
}

pub fn storage_format1() -> StorageLayout {
    StorageLayout {
        namespace: DISTRIBUTION.into(),
        format_version: 1,
        slots: vec![
            StorageSlot {
                index: 0,
                name: "keeperPeriod".into(),
                ty: "uint64".into(),
            },
            StorageSlot {
                index: 1,
                name: "lastPaidPeriod".into(),
                ty: "mapping(address=>uint64)".into(),
            },
            StorageSlot {
                index: 2,
                name: "eligibleCount".into(),
                ty: "uint32".into(),
            },
            StorageSlot {
                index: 3,
                name: "poolPerUnit".into(),
                ty: "uint128".into(),
            },
            StorageSlot {
                index: 4,
                name: "feeBps".into(),
                ty: "uint16".into(),
            },
        ],
    }
}

pub fn storage_format2() -> StorageLayout {
    StorageLayout {
        namespace: DISTRIBUTION.into(),
        format_version: 2,
        slots: vec![
            StorageSlot { index: 0, name: "keeperPeriod".into(), ty: "uint64".into() },
            StorageSlot { index: 1, name: "claimRecords".into(), ty: "mapping(address=>ClaimRecord{uint32 lastPaidPeriod,uint96 cumulativePaid,uint8 tier})".into() },
            StorageSlot { index: 2, name: "eligibleCount".into(), ty: "uint32".into() },
            StorageSlot { index: 3, name: "poolPerUnit".into(), ty: "uint128".into() },
            StorageSlot { index: 4, name: "feeBps".into(), ty: "uint16".into() },
        ],
    }
}

fn params(fee_bps: i64) -> BTreeMap<String, i64> {
    BTreeMap::from([
        ("feeBps".to_string(), fee_bps),
        ("poolPerUnit".to_string(), 1_000),
        ("periodLengthBlocks".to_string(), 1_000),
        ("eligibleCount".to_string(), 2_000),
    ])
}

fn f(
    name: &str,
    impl_id: &str,
    privileged: bool,
    body: &str,
    reads: &[&str],
    writes: &[&str],
) -> FunctionImpl {
    FunctionImpl {
        selector: selector_for(name),
        name: name.into(),
        impl_id: impl_id.into(),
        privileged,
        body: body.trim().into(),
        reads: reads.iter().map(|s| s.to_string()).collect(),
        writes: writes.iter().map(|s| s.to_string()).collect(),
    }
}

pub const CLAIM_BODY_2_3_0: &str = r#"
function claim(address w) external {
    uint64 period = keeperPeriod;                       // set by the keeper's rollover()
    require(lastPaidPeriod[w] != period, "already paid");
    uint256 amount = tier(w) * poolPerUnit;
    _pay(w, amount, feeBps);
    lastPaidPeriod[w] = period;
}"#;

pub const CLAIM_BODY_2_3_1: &str = r#"
function claim(address w) external {
    uint64 period = uint64(block.number / PERIOD_LENGTH); // derived from the block, not from keeperPeriod
    require(lastPaidPeriod[w] != period, "already paid");
    uint256 amount = tier(w) * poolPerUnit;
    _pay(w, amount, feeBps);
    lastPaidPeriod[w] = period;
}"#;

pub const CLAIM_BODY_2_3_1_FLAT: &str = r#"
function claim(address w) external {
    uint64 period = uint64(block.number / PERIOD_LENGTH);
    require(lastPaidPeriod[w] != period, "already paid");
    uint256 amount = 2 * poolPerUnit;                    // tier weight dropped
    _pay(w, amount, feeBps);
    lastPaidPeriod[w] = period;
}"#;

pub const CLAIM_BODY_2_3_1_DIVERTED: &str = r#"
function claim(address w) external {
    uint64 period = uint64(block.number / PERIOD_LENGTH);
    require(lastPaidPeriod[w] != period, "already paid");
    uint256 amount = tier(w) * poolPerUnit;
    _payTo(w, amount, feeBps, 0xA77ac0ffee0000000000000000000000000000d1); // fee rerouted to a hidden address
    lastPaidPeriod[w] = period;
}"#;

pub const CLAIM_BODY_3_0_0: &str = r#"
function claim(address w) external {
    uint32 period = uint32(block.number / PERIOD_LENGTH);
    ClaimRecord storage r = claimRecords[w];            // packed record replaces lastPaidPeriod
    require(r.lastPaidPeriod != period, "already paid");
    uint256 amount = tier(w) * poolPerUnit;
    _pay(w, amount, feeBps);
    r.lastPaidPeriod = period;
    r.cumulativePaid += uint96(amount);
    r.tier = tier(w);
}"#;

pub const CLAIM_FOR_BODY_2_3_1: &str = r#"
function claimFor(address[] calldata ws) external {
    uint64 period = uint64(block.number / PERIOD_LENGTH);
    bool[] memory ok = new bool[](ws.length);
    for (uint i; i < ws.length; i++) ok[i] = lastPaidPeriod[ws[i]] != period;   // eligibility snapshot
    for (uint i; i < ws.length; i++) if (ok[i]) _pay(ws[i], tier(ws[i]) * poolPerUnit, feeBps);
    for (uint i; i < ws.length; i++) lastPaidPeriod[ws[i]] = period;           // written after the loop
}"#;

pub const CLAIM_FOR_BODY_2_3_2: &str = r#"
function claimFor(address[] calldata ws) external {
    uint64 period = uint64(block.number / PERIOD_LENGTH);
    for (uint i; i < ws.length; i++) {
        address w = ws[i];
        if (lastPaidPeriod[w] == period) continue;       // checked and written per wallet
        lastPaidPeriod[w] = period;
        _pay(w, tier(w) * poolPerUnit, feeBps);
    }
}"#;

pub const CLAIM_FOR_BODY_3_0_0: &str = r#"
function claimFor(address[] calldata ws) external {
    uint32 period = uint32(block.number / PERIOD_LENGTH);
    for (uint i; i < ws.length; i++) {
        ClaimRecord storage r = claimRecords[ws[i]];
        if (r.lastPaidPeriod == period) continue;       // checked and written per wallet
        r.lastPaidPeriod = period;
        uint256 amount = tier(ws[i]) * poolPerUnit;
        r.cumulativePaid += uint96(amount);
        _pay(ws[i], amount, feeBps);
    }
}"#;

pub const ROLLOVER_BODY: &str = r#"
function rollover() external onlyKeeper {
    keeperPeriod = uint64(block.number / PERIOD_LENGTH);
}"#;

fn rollover() -> FunctionImpl {
    f(
        "rollover()",
        impls::ROLLOVER,
        true,
        ROLLOVER_BODY,
        &[],
        &["Distribution.keeperPeriod"],
    )
}

fn claim_for_faulty() -> FunctionImpl {
    f(
        "claimFor(address[])",
        impls::CLAIM_FOR_2_3_1,
        false,
        CLAIM_FOR_BODY_2_3_1,
        &[
            "Distribution.lastPaidPeriod",
            "Distribution.poolPerUnit",
            "Distribution.feeBps",
        ],
        &["Distribution.lastPaidPeriod"],
    )
}

fn claim_for_fixed() -> FunctionImpl {
    f(
        "claimFor(address[])",
        impls::CLAIM_FOR_2_3_2,
        false,
        CLAIM_FOR_BODY_2_3_2,
        &[
            "Distribution.lastPaidPeriod",
            "Distribution.poolPerUnit",
            "Distribution.feeBps",
        ],
        &["Distribution.lastPaidPeriod"],
    )
}

/// Batch claim on the format-2 layout (3.0.0): per-wallet check and write on `claimRecords`.
fn claim_for_fixed_v3() -> FunctionImpl {
    f(
        "claimFor(address[])",
        impls::CLAIM_FOR_2_3_2,
        false,
        CLAIM_FOR_BODY_3_0_0,
        &[
            "Distribution.claimRecords",
            "Distribution.poolPerUnit",
            "Distribution.feeBps",
        ],
        &["Distribution.claimRecords"],
    )
}

fn bundle(
    version: &str,
    functions: Vec<FunctionImpl>,
    storage: StorageLayout,
    fee_bps: i64,
    migration: Option<MigrationSpec>,
) -> ImplementationBundle {
    ImplementationBundle {
        format: BUNDLE_FORMAT.into(),
        package: DISTRIBUTION.into(),
        version: version.into(),
        functions,
        storage,
        parameters: params(fee_bps),
        roles: roles(),
        migration,
    }
}

/// Distribution 2.3.0: the production version with the rollover bug.
pub fn distribution_bundle_2_3_0() -> ImplementationBundle {
    bundle(
        "2.3.0",
        vec![
            f(
                "claim(address)",
                impls::CLAIM_2_3_0,
                false,
                CLAIM_BODY_2_3_0,
                &[
                    "Distribution.keeperPeriod",
                    "Distribution.lastPaidPeriod",
                    "Distribution.poolPerUnit",
                    "Distribution.feeBps",
                ],
                &["Distribution.lastPaidPeriod"],
            ),
            claim_for_faulty(),
            rollover(),
        ],
        storage_format1(),
        DEFAULT_FEE_BPS,
        None,
    )
}

/// Distribution 2.3.1: one function body changes (`claim`).
pub fn distribution_bundle_2_3_1() -> ImplementationBundle {
    bundle(
        "2.3.1",
        vec![
            f(
                "claim(address)",
                impls::CLAIM_2_3_1,
                false,
                CLAIM_BODY_2_3_1,
                &[
                    "Distribution.lastPaidPeriod",
                    "Distribution.poolPerUnit",
                    "Distribution.feeBps",
                ],
                &["Distribution.lastPaidPeriod"],
            ),
            claim_for_faulty(),
            rollover(),
        ],
        storage_format1(),
        DEFAULT_FEE_BPS,
        None,
    )
}

/// Failure mode 1 candidate: passes the uniqueness property, drops the tier weight.
pub fn distribution_bundle_2_3_1_flat() -> ImplementationBundle {
    let mut b = distribution_bundle_2_3_1();
    let c = b.function_mut("claim(address)").expect("claim");
    c.impl_id = impls::CLAIM_2_3_1_FLAT.into();
    c.body = CLAIM_BODY_2_3_1_FLAT.trim().into();
    b
}

/// Substitution attack payload: same interface, fee rerouted.
pub fn distribution_bundle_2_3_1_diverted() -> ImplementationBundle {
    let mut b = distribution_bundle_2_3_1();
    let c = b.function_mut("claim(address)").expect("claim");
    c.impl_id = impls::CLAIM_2_3_1_DIVERTED.into();
    c.body = CLAIM_BODY_2_3_1_DIVERTED.trim().into();
    b
}

/// Case B candidate: code unchanged, `feeBps` 100 -> 250.
pub fn distribution_bundle_2_3_1_fee(fee_bps: i64) -> ImplementationBundle {
    let mut b = distribution_bundle_2_3_1();
    b.parameters.insert("feeBps".into(), fee_bps);
    b
}

/// Case C candidate: 3.0.0 with a storage migration to packed claim records.
pub fn distribution_bundle_3_0_0() -> ImplementationBundle {
    bundle(
        "3.0.0",
        vec![
            f("claim(address)", impls::CLAIM_3_0_0, false, CLAIM_BODY_3_0_0, &["Distribution.claimRecords", "Distribution.poolPerUnit", "Distribution.feeBps"], &["Distribution.claimRecords"]),
            claim_for_fixed_v3(),
            rollover(),
        ],
        storage_format2(),
        DEFAULT_FEE_BPS,
        Some(MigrationSpec {
            id: "migrate-claim-records-v1-to-v2".into(),
            description: "For every wallet with a lastPaidPeriod entry, write a packed ClaimRecord {lastPaidPeriod, cumulativePaid, tier} into slot 1 and clear the old mapping.".into(),
            from_format: 1,
            to_format: 2,
            proposer_reversibility_claim: "Reinstalling 2.3.1 restores prior behaviour.".into(),
        }),
    )
}

/// Recovery proposal for Case C: reverse migration back to format 1.
pub fn distribution_bundle_3_0_1_reverse() -> ImplementationBundle {
    bundle(
        "3.0.1-reverse",
        vec![
            f("claim(address)", impls::CLAIM_3_0_1_REVERSE, false, CLAIM_BODY_2_3_1, &["Distribution.lastPaidPeriod", "Distribution.poolPerUnit", "Distribution.feeBps"], &["Distribution.lastPaidPeriod"]),
            claim_for_fixed(),
            rollover(),
        ],
        storage_format1(),
        DEFAULT_FEE_BPS,
        Some(MigrationSpec {
            id: "migrate-claim-records-v2-to-v1".into(),
            description: "For every ClaimRecord, write lastPaidPeriod back into the uint64 mapping and clear the records. cumulativePaid is dropped; payouts made under 3.0.0 remain paid.".into(),
            from_format: 2,
            to_format: 1,
            proposer_reversibility_claim: "Restores the format-1 layout; does not restore balances.".into(),
        }),
    )
}

/// Case D candidate: 2.3.2 fixes the batch fault while it is being exploited.
pub fn distribution_bundle_2_3_2() -> ImplementationBundle {
    let mut b = distribution_bundle_2_3_1();
    b.version = "2.3.2".into();
    let c = b.function_mut("claimFor(address[])").expect("claimFor");
    *c = claim_for_fixed();
    b
}

/// A minimal unified diff of two function bodies (line-based, for display).
pub fn body_diff(name: &str, before: &str, after: &str) -> String {
    let mut out = format!("--- {name} (installed)\n+++ {name} (candidate)\n");
    let b: Vec<&str> = before.lines().collect();
    let a: Vec<&str> = after.lines().collect();
    let n = b.len().max(a.len());
    for i in 0..n {
        match (b.get(i), a.get(i)) {
            (Some(x), Some(y)) if x == y => out.push_str(&format!("  {x}\n")),
            (Some(x), Some(y)) => {
                out.push_str(&format!("- {x}\n"));
                out.push_str(&format!("+ {y}\n"));
            }
            (Some(x), None) => out.push_str(&format!("- {x}\n")),
            (None, Some(y)) => out.push_str(&format!("+ {y}\n")),
            (None, None) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundles_are_deterministic_and_parse_back() {
        for b in [
            distribution_bundle_2_3_0(),
            distribution_bundle_2_3_1(),
            distribution_bundle_2_3_1_flat(),
            distribution_bundle_2_3_1_diverted(),
            distribution_bundle_2_3_1_fee(CASE_B_FEE_BPS),
            distribution_bundle_3_0_0(),
            distribution_bundle_3_0_1_reverse(),
            distribution_bundle_2_3_2(),
        ] {
            let bytes = b.payload_bytes().unwrap();
            assert_eq!(bytes, b.payload_bytes().unwrap());
            assert_eq!(ImplementationBundle::from_payload_bytes(&bytes).unwrap(), b);
        }
    }

    #[test]
    fn only_claim_body_differs_between_2_3_0_and_2_3_1() {
        let a = distribution_bundle_2_3_0();
        let b = distribution_bundle_2_3_1();
        assert_eq!(a.storage, b.storage);
        assert_eq!(a.parameters, b.parameters);
        assert_eq!(a.roles, b.roles);
        let changed: Vec<&str> = a
            .functions
            .iter()
            .zip(b.functions.iter())
            .filter(|(x, y)| x != y)
            .map(|(x, _)| x.name.as_str())
            .collect();
        assert_eq!(changed, vec!["claim(address)"]);
    }
}
