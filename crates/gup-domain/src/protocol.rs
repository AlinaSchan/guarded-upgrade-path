//! The local protocol model.
//!
//! A small diamond-style system with one package, `Distribution`, that pays
//! eligible wallets once per period. The model is deterministic and runs
//! entirely in memory. It is **not** a chain, a fork or an EVM; see
//! `docs/SIMULATION_SCOPE.md`.
//!
//! Implementation behaviour is keyed by `impl_id` (a behaviour registry).
//! Installing a payload swaps the `impl_id` behind a selector; the replay
//! engine then interprets claims with that behaviour.

use crate::artifact::{FunctionEntry, ImplementationBundle, RoleSet, StorageLayout};
use crate::canonical::Commitment;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const DISTRIBUTION: &str = "Distribution";
pub const BLOCKS_PER_PERIOD: u64 = 1_000;

/// Known claim behaviours. Anything not listed here cannot be executed by the
/// local model, which is itself a (documented) limitation of the demo.
pub mod impls {
    /// Keeper-period based eligibility (the bug): `lastPaidPeriod[w] == keeperPeriod` check.
    pub const CLAIM_2_3_0: &str = "distribution.claim/2.3.0";
    /// Block-derived period eligibility (the fix).
    pub const CLAIM_2_3_1: &str = "distribution.claim/2.3.1";
    /// Passes the uniqueness property but ignores the tier weight (failure mode 1).
    pub const CLAIM_2_3_1_FLAT: &str = "distribution.claim/2.3.1-flat-amount";
    /// Block-derived period, but routes the protocol fee to a hidden address (substitution attack).
    pub const CLAIM_2_3_1_DIVERTED: &str = "distribution.claim/2.3.1-diverted";
    /// Packed claim records, storage format 2 (Case C).
    pub const CLAIM_3_0_0: &str = "distribution.claim/3.0.0";
    /// Batch claim with the in-batch duplicate fault (Case D).
    pub const CLAIM_FOR_2_3_1: &str = "distribution.claimFor/2.3.1";
    /// Batch claim, fixed.
    pub const CLAIM_FOR_2_3_2: &str = "distribution.claimFor/2.3.2";
    /// Keeper rollover: advances `keeperPeriod`.
    pub const ROLLOVER: &str = "distribution.rollover/1";
    /// Reverse migration (format 2 -> 1), used by recovery proposals.
    pub const CLAIM_3_0_1_REVERSE: &str = "distribution.claim/3.0.1-reverse";
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageState {
    pub name: String,
    pub version: String,
    /// selector -> declared function
    pub functions: BTreeMap<String, FunctionEntry>,
    pub storage: StorageLayout,
    pub parameters: BTreeMap<String, i64>,
    /// The implementation bundle currently installed (kept so that structural
    /// checks and recovery can compare against the real prior code).
    pub installed: ImplementationBundle,
}

impl PackageState {
    pub fn from_bundle(bundle: &ImplementationBundle) -> Self {
        PackageState {
            name: bundle.package.clone(),
            version: bundle.version.clone(),
            functions: bundle
                .functions
                .iter()
                .map(|f| (f.selector.clone(), f.entry()))
                .collect(),
            storage: bundle.storage.clone(),
            parameters: bundle.parameters.clone(),
            installed: bundle.clone(),
        }
    }

    pub fn impl_for(&self, name: &str) -> Option<&str> {
        self.functions
            .values()
            .find(|f| f.name == name)
            .map(|f| f.impl_id.as_str())
    }

    pub fn param(&self, key: &str) -> i64 {
        *self.parameters.get(key).unwrap_or(&0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PauseState {
    pub protocol: bool,
    pub packages: BTreeSet<String>,
    pub functions: BTreeSet<String>,
}

impl PauseState {
    pub fn is_paused(&self, package: &str, function: &str) -> bool {
        self.protocol || self.packages.contains(package) || self.functions.contains(function)
    }
    pub fn any(&self) -> bool {
        self.protocol || !self.packages.is_empty() || !self.functions.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClaimRecord {
    pub last_paid_period: u64,
    pub cumulative_paid: i64,
    pub tier: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payout {
    pub wallet: u32,
    pub amount_gross: i64,
    pub amount_net: i64,
    pub fee: i64,
    pub block: u64,
    /// Period derived from the block (the ground truth for uniqueness).
    pub block_period: u64,
    /// Period the implementation recorded for this payout.
    pub recorded_period: u64,
    pub impl_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PeriodStat {
    pub period: u64,
    pub claims: u64,
    pub paid: u64,
    pub rejected: u64,
    pub gross: i64,
    pub net: i64,
    pub fee: i64,
    pub duplicates: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicatePayout {
    pub wallet: u32,
    pub block_period: u64,
    pub first_block: u64,
    pub second_block: u64,
    pub amount_gross: i64,
}

/// Persistent state of the Distribution package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributionState {
    /// 1 = `lastPaidPeriod` mapping; 2 = packed `claimRecords` (Case C).
    pub format_version: u32,
    /// The scheduler's period id, advanced by the keeper's rollover tx.
    pub keeper_period: u64,
    pub last_paid_period: BTreeMap<u32, u64>,
    pub claim_records: BTreeMap<u32, ClaimRecord>,
    pub eligible_wallets: u32,
    pub pool_per_unit: i64,
    pub treasury: i64,
    pub fee_collected: i64,
    /// Fee diverted by a malicious implementation (only ever non-zero if an
    /// unverified payload were executed; the executor refuses that).
    pub diverted: i64,
    pub payout_count: u64,
    pub per_period: BTreeMap<u64, PeriodStat>,
    pub per_wallet_paid: BTreeMap<u32, i64>,
    pub duplicates: Vec<DuplicatePayout>,
    /// Period of the payouts currently tracked in `paid_in_period`.
    pub paid_marker: u64,
    /// wallet -> first payout block within `paid_marker`, for duplicate detection.
    pub paid_in_period: BTreeMap<u32, u64>,
    pub recent_payouts: Vec<Payout>,
    /// Count of reads where the installed code could not interpret the stored format.
    pub misreads: u64,
}

impl DistributionState {
    pub fn new(eligible_wallets: u32, pool_per_unit: i64, treasury: i64) -> Self {
        DistributionState {
            format_version: 1,
            keeper_period: 0,
            last_paid_period: BTreeMap::new(),
            claim_records: BTreeMap::new(),
            eligible_wallets,
            pool_per_unit,
            treasury,
            fee_collected: 0,
            diverted: 0,
            payout_count: 0,
            per_period: BTreeMap::new(),
            per_wallet_paid: BTreeMap::new(),
            duplicates: Vec::new(),
            paid_marker: 0,
            paid_in_period: BTreeMap::new(),
            recent_payouts: Vec::new(),
            misreads: 0,
        }
    }

    pub fn tier(wallet: u32) -> u8 {
        (wallet % 3 + 1) as u8
    }

    pub fn total_gross(&self) -> i64 {
        self.per_period.values().map(|p| p.gross).sum()
    }

    pub fn total_duplicates(&self) -> usize {
        self.duplicates.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    Transfer,
    PermissionUsed,
    Integration,
    UserAction,
    Pause,
}

/// Something that happened outside the code and cannot be undone by
/// reinstalling an implementation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalEffect {
    pub kind: EffectKind,
    pub block: u64,
    pub description: String,
    pub amount: Option<i64>,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeRecord {
    pub proposal_id: String,
    pub package: String,
    pub from_version: String,
    pub to_version: String,
    pub commitment: Commitment,
    pub block: u64,
    /// The bundle that was installed before this upgrade (for recovery layer 1).
    pub prior_bundle: ImplementationBundle,
    pub prior_format_version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolState {
    pub block: u64,
    pub period_length: u64,
    pub packages: BTreeMap<String, PackageState>,
    pub roles: RoleSet,
    pub pause: PauseState,
    pub distribution: DistributionState,
    pub effects: Vec<ExternalEffect>,
    pub history: Vec<UpgradeRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimOutcome {
    Paid {
        amount_gross: i64,
        amount_net: i64,
        duplicate: bool,
    },
    Rejected {
        reason: String,
    },
    Misread {
        detail: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("package {0} is not installed")]
    NoSuchPackage(String),
    #[error("function {0} is not installed")]
    NoSuchFunction(String),
    #[error("implementation {0} is not known to the local model")]
    UnknownImpl(String),
}

impl ProtocolState {
    pub fn new(roles: RoleSet, distribution_bundle: &ImplementationBundle) -> Self {
        let mut packages = BTreeMap::new();
        packages.insert(
            DISTRIBUTION.to_string(),
            PackageState::from_bundle(distribution_bundle),
        );
        ProtocolState {
            block: 0,
            period_length: BLOCKS_PER_PERIOD,
            packages,
            roles: roles.normalized(),
            pause: PauseState::default(),
            distribution: DistributionState::new(2_000, 1_000, 5_000_000_000),
            effects: Vec::new(),
            history: Vec::new(),
        }
    }

    pub fn period_of(&self, block: u64) -> u64 {
        block / self.period_length
    }

    pub fn distribution_package(&self) -> &PackageState {
        self.packages
            .get(DISTRIBUTION)
            .expect("Distribution package always installed")
    }

    pub fn distribution_package_mut(&mut self) -> &mut PackageState {
        self.packages
            .get_mut(DISTRIBUTION)
            .expect("Distribution package always installed")
    }

    pub fn fee_bps(&self) -> i64 {
        self.distribution_package().param("feeBps")
    }

    pub fn record_effect(
        &mut self,
        kind: EffectKind,
        description: impl Into<String>,
        amount: Option<i64>,
        count: u64,
    ) {
        self.effects.push(ExternalEffect {
            kind,
            block: self.block,
            description: description.into(),
            amount,
            count,
        });
    }

    /// Keeper rollover transaction: advances the scheduler's period id.
    pub fn rollover(&mut self, block: u64) {
        self.block = block;
        self.distribution.keeper_period = self.period_of(block);
    }

    /// Read `lastPaidPeriod[w]` the way format-1 code does. If the state has
    /// been migrated to format 2, format-1 code misreads the packed record.
    fn read_last_paid_format1(&mut self, wallet: u32) -> Result<Option<u64>, String> {
        match self.distribution.format_version {
            1 => Ok(self.distribution.last_paid_period.get(&wallet).copied()),
            _ => {
                self.distribution.misreads += 1;
                // The packed record occupies the slot that format-1 code reads as uint64.
                let garbled = self.distribution.claim_records.get(&wallet).map(|r| {
                    (r.cumulative_paid as u64) << 8 | r.tier as u64 | r.last_paid_period << 40
                });
                Err(format!(
                    "slot read as uint64 lastPaidPeriod but holds a packed ClaimRecord (raw {})",
                    garbled
                        .map(|g| g.to_string())
                        .unwrap_or_else(|| "empty".into())
                ))
            }
        }
    }

    fn pay(
        &mut self,
        wallet: u32,
        block: u64,
        recorded_period: u64,
        impl_id: &str,
        gross: i64,
        divert_fee: bool,
    ) -> ClaimOutcome {
        let fee = gross * self.fee_bps() / 10_000;
        let net = gross - fee;
        let block_period = self.period_of(block);
        let d = &mut self.distribution;
        d.treasury -= gross;
        if divert_fee {
            d.diverted += fee;
        } else {
            d.fee_collected += fee;
        }
        d.payout_count += 1;
        *d.per_wallet_paid.entry(wallet).or_insert(0) += gross;
        let stat = d
            .per_period
            .entry(block_period)
            .or_insert_with(|| PeriodStat {
                period: block_period,
                ..Default::default()
            });
        stat.paid += 1;
        stat.gross += gross;
        stat.net += net;
        stat.fee += fee;
        if d.paid_marker != block_period {
            d.paid_marker = block_period;
            d.paid_in_period.clear();
        }
        let duplicate = if let Some(first) = d.paid_in_period.get(&wallet).copied() {
            stat.duplicates += 1;
            d.duplicates.push(DuplicatePayout {
                wallet,
                block_period,
                first_block: first,
                second_block: block,
                amount_gross: gross,
            });
            true
        } else {
            d.paid_in_period.insert(wallet, block);
            false
        };
        d.recent_payouts.push(Payout {
            wallet,
            amount_gross: gross,
            amount_net: net,
            fee,
            block,
            block_period,
            recorded_period,
            impl_id: impl_id.to_string(),
        });
        if d.recent_payouts.len() > 40 {
            d.recent_payouts.remove(0);
        }
        ClaimOutcome::Paid {
            amount_gross: gross,
            amount_net: net,
            duplicate,
        }
    }

    fn note_claim(&mut self, block: u64, paid: bool) {
        let p = self.period_of(block);
        let stat = self
            .distribution
            .per_period
            .entry(p)
            .or_insert_with(|| PeriodStat {
                period: p,
                ..Default::default()
            });
        stat.claims += 1;
        if !paid {
            stat.rejected += 1;
        }
    }

    /// Process a single `claim(address)` call at `block` using the installed implementation.
    pub fn claim(&mut self, wallet: u32, block: u64) -> Result<ClaimOutcome, ProtocolError> {
        let impl_id = self
            .distribution_package()
            .impl_for("claim(address)")
            .ok_or_else(|| ProtocolError::NoSuchFunction("claim(address)".into()))?
            .to_string();
        self.claim_with(&impl_id, wallet, block)
    }

    /// Process a claim with an explicit behaviour (used by replays of candidates).
    pub fn claim_with(
        &mut self,
        impl_id: &str,
        wallet: u32,
        block: u64,
    ) -> Result<ClaimOutcome, ProtocolError> {
        self.block = block;
        if self.pause.is_paused(DISTRIBUTION, "claim(address)") {
            self.note_claim(block, false);
            return Ok(ClaimOutcome::Rejected {
                reason: "paused".into(),
            });
        }
        if wallet >= self.distribution.eligible_wallets {
            self.note_claim(block, false);
            return Ok(ClaimOutcome::Rejected {
                reason: "not eligible".into(),
            });
        }
        let tier = DistributionState::tier(wallet) as i64;
        let unit = self.distribution.pool_per_unit;
        let block_period = self.period_of(block);
        let outcome = match impl_id {
            impls::CLAIM_2_3_0 => {
                // Eligibility keyed by the keeper's period id. Between a period
                // boundary and the keeper's rollover tx, the id is stale.
                let period = self.distribution.keeper_period;
                match self.read_last_paid_format1(wallet) {
                    Err(detail) => ClaimOutcome::Misread { detail },
                    Ok(Some(p)) if p == period => ClaimOutcome::Rejected {
                        reason: format!("already paid for keeper period {period}"),
                    },
                    Ok(_) => {
                        self.distribution.last_paid_period.insert(wallet, period);
                        self.pay(wallet, block, period, impl_id, tier * unit, false)
                    }
                }
            }
            impls::CLAIM_2_3_1 | impls::CLAIM_2_3_1_FLAT | impls::CLAIM_2_3_1_DIVERTED => {
                // Eligibility keyed by the block-derived period.
                let period = block_period;
                match self.read_last_paid_format1(wallet) {
                    Err(detail) => ClaimOutcome::Misread { detail },
                    Ok(Some(p)) if p == period => ClaimOutcome::Rejected {
                        reason: format!("already paid for period {period}"),
                    },
                    Ok(_) => {
                        self.distribution.last_paid_period.insert(wallet, period);
                        let gross = if impl_id == impls::CLAIM_2_3_1_FLAT {
                            2 * unit
                        } else {
                            tier * unit
                        };
                        self.pay(
                            wallet,
                            block,
                            period,
                            impl_id,
                            gross,
                            impl_id == impls::CLAIM_2_3_1_DIVERTED,
                        )
                    }
                }
            }
            impls::CLAIM_3_0_0 => {
                if self.distribution.format_version != 2 {
                    self.distribution.misreads += 1;
                    ClaimOutcome::Misread {
                        detail: "format-2 code reading format-1 state".into(),
                    }
                } else {
                    let period = block_period;
                    let rec = self
                        .distribution
                        .claim_records
                        .get(&wallet)
                        .cloned()
                        .unwrap_or_default();
                    if rec.last_paid_period == period && rec.cumulative_paid > 0 {
                        ClaimOutcome::Rejected {
                            reason: format!("already paid for period {period}"),
                        }
                    } else {
                        let gross = tier * unit;
                        let outcome = self.pay(wallet, block, period, impl_id, gross, false);
                        let rec = self.distribution.claim_records.entry(wallet).or_default();
                        rec.last_paid_period = period;
                        rec.cumulative_paid += gross;
                        rec.tier = tier as u8;
                        outcome
                    }
                }
            }
            impls::CLAIM_3_0_1_REVERSE => {
                // Behaves like 2.3.1 on format-1 state after the reverse migration.
                let period = block_period;
                match self.read_last_paid_format1(wallet) {
                    Err(detail) => ClaimOutcome::Misread { detail },
                    Ok(Some(p)) if p == period => ClaimOutcome::Rejected {
                        reason: format!("already paid for period {period}"),
                    },
                    Ok(_) => {
                        self.distribution.last_paid_period.insert(wallet, period);
                        self.pay(wallet, block, period, impl_id, tier * unit, false)
                    }
                }
            }
            other => return Err(ProtocolError::UnknownImpl(other.to_string())),
        };
        let paid = matches!(outcome, ClaimOutcome::Paid { .. });
        self.note_claim(block, paid);
        Ok(outcome)
    }

    /// Process a `claimFor(address[])` batch call (Case D).
    pub fn claim_batch(
        &mut self,
        wallets: &[u32],
        block: u64,
    ) -> Result<Vec<ClaimOutcome>, ProtocolError> {
        let impl_id = self
            .distribution_package()
            .impl_for("claimFor(address[])")
            .ok_or_else(|| ProtocolError::NoSuchFunction("claimFor(address[])".into()))?
            .to_string();
        self.claim_batch_with(&impl_id, wallets, block)
    }

    pub fn claim_batch_with(
        &mut self,
        impl_id: &str,
        wallets: &[u32],
        block: u64,
    ) -> Result<Vec<ClaimOutcome>, ProtocolError> {
        self.block = block;
        if self.pause.is_paused(DISTRIBUTION, "claimFor(address[])") {
            return Ok(wallets
                .iter()
                .map(|_| ClaimOutcome::Rejected {
                    reason: "paused".into(),
                })
                .collect());
        }
        let mut out = Vec::with_capacity(wallets.len());
        match impl_id {
            impls::CLAIM_FOR_2_3_1 => {
                // Fault: eligibility is read for the whole batch first and
                // lastPaidPeriod is written only after the loop, so a wallet
                // repeated inside one batch is paid once per repetition.
                let period = self.period_of(block);
                let mut eligible = Vec::new();
                for &w in wallets {
                    let already =
                        self.distribution.last_paid_period.get(&w).copied() == Some(period);
                    eligible.push(!already && w < self.distribution.eligible_wallets);
                }
                for (i, &w) in wallets.iter().enumerate() {
                    if eligible[i] {
                        let gross =
                            DistributionState::tier(w) as i64 * self.distribution.pool_per_unit;
                        out.push(self.pay(w, block, period, impl_id, gross, false));
                        self.note_claim(block, true);
                    } else {
                        out.push(ClaimOutcome::Rejected {
                            reason: "not eligible or already paid".into(),
                        });
                        self.note_claim(block, false);
                    }
                }
                for &w in wallets {
                    self.distribution.last_paid_period.insert(w, period);
                }
            }
            impls::CLAIM_FOR_2_3_2 => {
                // Per-wallet check and write, through whatever claim behaviour is installed
                // (format-1 logic under 2.3.x, packed records under 3.0.0).
                let claim_impl = self
                    .distribution_package()
                    .impl_for("claim(address)")
                    .unwrap_or(impls::CLAIM_2_3_1)
                    .to_string();
                for &w in wallets {
                    out.push(self.claim_with(&claim_impl, w, block)?);
                }
            }
            other => return Err(ProtocolError::UnknownImpl(other.to_string())),
        }
        Ok(out)
    }

    /// Transform persistent state from format 1 to format 2 (Case C migration).
    pub fn migrate_to_format2(&mut self) -> u64 {
        let d = &mut self.distribution;
        let mut migrated = 0u64;
        for (w, p) in d.last_paid_period.iter() {
            d.claim_records.insert(
                *w,
                ClaimRecord {
                    last_paid_period: *p,
                    cumulative_paid: *d.per_wallet_paid.get(w).unwrap_or(&0),
                    tier: DistributionState::tier(*w),
                },
            );
            migrated += 1;
        }
        d.last_paid_period.clear();
        d.format_version = 2;
        migrated
    }

    /// Transform persistent state from format 2 back to format 1 (recovery migration).
    pub fn migrate_to_format1(&mut self) -> u64 {
        let d = &mut self.distribution;
        let mut migrated = 0u64;
        for (w, r) in d.claim_records.iter() {
            d.last_paid_period.insert(*w, r.last_paid_period);
            migrated += 1;
        }
        d.claim_records.clear();
        d.format_version = 1;
        migrated
    }

    /// Install a bundle as the Distribution implementation. Runs the declared
    /// migration if present. This is the only way code or parameters change.
    pub fn install(
        &mut self,
        bundle: &ImplementationBundle,
        proposal_id: &str,
        commitment: Commitment,
    ) -> Result<InstallReport, ProtocolError> {
        if bundle.package != DISTRIBUTION {
            return Err(ProtocolError::NoSuchPackage(bundle.package.clone()));
        }
        for f in &bundle.functions {
            let known = [
                impls::CLAIM_2_3_0,
                impls::CLAIM_2_3_1,
                impls::CLAIM_2_3_1_FLAT,
                impls::CLAIM_2_3_1_DIVERTED,
                impls::CLAIM_3_0_0,
                impls::CLAIM_FOR_2_3_1,
                impls::CLAIM_FOR_2_3_2,
                impls::ROLLOVER,
                impls::CLAIM_3_0_1_REVERSE,
            ];
            if !known.contains(&f.impl_id.as_str()) {
                return Err(ProtocolError::UnknownImpl(f.impl_id.clone()));
            }
        }
        let prior = self.distribution_package().installed.clone();
        let prior_format = self.distribution.format_version;
        let mut migrated_entries = 0;
        if let Some(m) = &bundle.migration {
            if m.from_format == 1 && m.to_format == 2 {
                migrated_entries = self.migrate_to_format2();
            } else if m.from_format == 2 && m.to_format == 1 {
                migrated_entries = self.migrate_to_format1();
            }
        }
        let pkg = self.distribution_package_mut();
        *pkg = PackageState::from_bundle(bundle);
        self.history.push(UpgradeRecord {
            proposal_id: proposal_id.to_string(),
            package: DISTRIBUTION.to_string(),
            from_version: prior.version.clone(),
            to_version: bundle.version.clone(),
            commitment,
            block: self.block,
            prior_bundle: prior.clone(),
            prior_format_version: prior_format,
        });
        Ok(InstallReport {
            from_version: prior.version,
            to_version: bundle.version.clone(),
            migrated_entries,
            format_version: self.distribution.format_version,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallReport {
    pub from_version: String,
    pub to_version: String,
    pub migrated_entries: u64,
    pub format_version: u32,
}
