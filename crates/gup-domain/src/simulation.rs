//! Deterministic local replay model.
//!
//! A seeded claim stream is replayed through a copy of the protocol model,
//! optionally after installing a candidate bundle on that copy (which runs
//! the candidate's migration, if any). Everything here is a **simulation**:
//! it establishes what happened in the covered cases and nothing beyond
//! them. No chain fork, no real transactions, no real bytecode. The fixture
//! figures (41,200 claims, 30 periods, 14 affected wallets) are illustrative
//! and reproduce the hypothetical example in the review draft.

use crate::artifact::ImplementationBundle;
use crate::canonical::Commitment;
use crate::protocol::{
    ClaimOutcome, DuplicatePayout, PeriodStat, ProtocolState, BLOCKS_PER_PERIOD,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// xorshift64* — small, fast, fully deterministic.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        if v.len() < 2 {
            return;
        }
        for i in (1..v.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            v.swap(i, j);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ClaimKind {
    Claim { wallet: u32 },
    Rollover,
    Batch { wallets: Vec<u32> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimEvent {
    pub block: u64,
    #[serde(flatten)]
    pub kind: ClaimKind,
}

/// A deterministic stream of claim transactions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimStream {
    pub name: String,
    pub seed: u64,
    pub periods: u64,
    pub period_length: u64,
    pub first_block: u64,
    pub last_block: u64,
    pub claim_count: usize,
    pub description: String,
    /// Periods on which the keeper's rollover transaction lagged, and by how many blocks.
    pub keeper_lag: BTreeMap<u64, u64>,
    /// Wallets deliberately placed in the lag window (the engineered trigger).
    pub lag_window_wallets: BTreeMap<u64, Vec<u32>>,
    pub events: Vec<ClaimEvent>,
}

impl ClaimStream {
    pub fn claims(&self) -> usize {
        self.events
            .iter()
            .map(|e| match &e.kind {
                ClaimKind::Claim { .. } => 1,
                ClaimKind::Batch { wallets } => wallets.len(),
                ClaimKind::Rollover => 0,
            })
            .sum()
    }

    fn sort_events(&mut self) {
        // Stable order: by block, rollover first within a block.
        self.events.sort_by(|a, b| {
            a.block.cmp(&b.block).then_with(|| {
                let ra = matches!(a.kind, ClaimKind::Rollover);
                let rb = matches!(b.kind, ClaimKind::Rollover);
                rb.cmp(&ra)
            })
        });
    }
}

pub const FIXTURE_CLAIMS: usize = 41_200;
pub const FIXTURE_PERIODS: u64 = 30;
pub const FIXTURE_AFFECTED_WALLETS: usize = 14;
pub const FIXTURE_LAG_PERIODS: [u64; 3] = [27, 28, 29];
pub const FIXTURE_SEED: u64 = 0x5eed_d15d_41b0_0001;

/// The 30-period, 41,200-claim fixture used for the Distribution 2.3.0 → 2.3.1
/// example. On three periods the keeper's rollover transaction arrives three
/// blocks late; fourteen wallets claim inside that window and again after the
/// rollover. Under 2.3.0 each of them is paid twice for one period.
pub fn distribution_fixture_stream() -> ClaimStream {
    build_stream(
        FIXTURE_SEED,
        FIXTURE_PERIODS,
        FIXTURE_CLAIMS,
        &FIXTURE_LAG_PERIODS,
        3,
        true,
    )
}

/// Wallets chosen for the engineered trigger: tiers picked so that the
/// overpayment lands near 0.3% of daily distribution (7 tier-1, 3 tier-2,
/// 4 tier-3 wallets). Tier = wallet % 3 + 1.
pub fn trigger_wallets() -> Vec<u32> {
    vec![
        300, 600, 900, 1200, 1500, 1800, 1890, 301, 601, 901, 302, 602, 902, 1202,
    ]
}

/// Build a stream. `lag_periods` get a keeper rollover `lag` blocks after the
/// boundary and (when `engineer_trigger`) a fixed set of wallets claiming
/// inside the window and again later.
pub fn build_stream(
    seed: u64,
    periods: u64,
    total_claims: usize,
    lag_periods: &[u64],
    lag: u64,
    engineer_trigger: bool,
) -> ClaimStream {
    let period_length = BLOCKS_PER_PERIOD;
    let eligible = 2_000u32;
    let mut rng = Rng::new(seed);

    let mut lag_window_wallets: BTreeMap<u64, Vec<u32>> = BTreeMap::new();
    if engineer_trigger && !lag_periods.is_empty() && lag > 0 {
        let chosen = trigger_wallets();
        let per = (chosen.len() + lag_periods.len() - 1) / lag_periods.len();
        for (i, chunk) in chosen.chunks(per).enumerate() {
            if let Some(p) = lag_periods.get(i) {
                lag_window_wallets.insert(*p, chunk.to_vec());
            }
        }
    }
    let trigger_extra: usize = 2 * lag_window_wallets.values().map(Vec::len).sum::<usize>();
    let base_total = total_claims.saturating_sub(trigger_extra);
    let per_period = base_total / periods.max(1) as usize;
    let remainder = base_total % periods.max(1) as usize;

    let mut events = Vec::with_capacity(total_claims + periods as usize);
    let mut keeper_lag = BTreeMap::new();

    for p in 0..periods {
        let start = p * period_length;
        let lag_here = if lag_periods.contains(&p) { lag } else { 0 };
        keeper_lag.insert(p, lag_here);
        events.push(ClaimEvent {
            block: start + lag_here,
            kind: ClaimKind::Rollover,
        });

        let window: Vec<u32> = lag_window_wallets.get(&p).cloned().unwrap_or_default();
        let excluded_next: Vec<u32> = lag_window_wallets
            .get(&(p + 1))
            .cloned()
            .unwrap_or_default();

        let mut count = per_period + if (p as usize) < remainder { 1 } else { 0 };
        let mut pool: Vec<u32> = (0..eligible)
            .filter(|w| !window.contains(w) && !excluded_next.contains(w))
            .collect();
        rng.shuffle(&mut pool);
        count = count.min(pool.len());
        let span = period_length.saturating_sub(lag_here + 2).max(1);
        let mut blocks: Vec<u64> = (0..count)
            .map(|_| start + lag_here + 1 + rng.below(span))
            .collect();
        blocks.sort_unstable();
        for (i, w) in pool.into_iter().take(count).enumerate() {
            events.push(ClaimEvent {
                block: blocks[i],
                kind: ClaimKind::Claim { wallet: w },
            });
        }
        // Trigger wallets: once inside the lag window, once well after the rollover.
        for (i, w) in window.iter().enumerate() {
            let in_window = start + (i as u64 % lag_here.max(1));
            events.push(ClaimEvent {
                block: in_window,
                kind: ClaimKind::Claim { wallet: *w },
            });
            let later = start
                + lag_here
                + 50
                + rng.below(period_length.saturating_sub(lag_here + 100).max(1));
            events.push(ClaimEvent {
                block: later,
                kind: ClaimKind::Claim { wallet: *w },
            });
        }
    }
    let mut stream = ClaimStream {
        name: format!("distribution-{periods}p-seed{seed:x}"),
        seed,
        periods,
        period_length,
        first_block: 0,
        last_block: periods * period_length - 1,
        claim_count: 0,
        description: String::new(),
        keeper_lag,
        lag_window_wallets,
        events,
    };
    stream.sort_events();
    // In-window trigger claims must precede the rollover of their block: the
    // rollover sits at start+lag, trigger claims sit in [start, start+lag).
    stream.claim_count = stream.claims();
    stream.description = format!(
        "{} claim transactions over {periods} periods of {period_length} blocks; keeper rollover lagged by {lag} blocks on periods {:?}. Deterministic synthetic stream (seed {seed:#x}), not production data.",
        stream.claim_count, lag_periods
    );
    stream.name = format!(
        "distribution-{periods}p-{}c-seed{seed:x}",
        stream.claim_count
    );
    stream
}

/// Case D: batches with a repeated wallet, submitted by an attacker.
pub fn exploit_stream(start_block: u64, blocks: u64) -> ClaimStream {
    let mut events = vec![ClaimEvent {
        block: start_block,
        kind: ClaimKind::Rollover,
    }];
    for i in 0..blocks {
        let b = start_block + i;
        // A fresh attacker-controlled wallet repeated four times inside one batch, plus two legitimate wallets.
        let w = 1_999 - i as u32;
        events.push(ClaimEvent {
            block: b,
            kind: ClaimKind::Batch {
                wallets: vec![
                    w,
                    w,
                    w,
                    w,
                    (10 + i as u32) % 1_900,
                    (11 + i as u32 * 7) % 1_900,
                ],
            },
        });
    }
    let mut s = ClaimStream {
        name: format!("exploit-{blocks}b-from-{start_block}"),
        seed: 0,
        periods: 1,
        period_length: BLOCKS_PER_PERIOD,
        first_block: start_block,
        last_block: start_block + blocks - 1,
        claim_count: 0,
        description: format!("{blocks} attacker batches, each repeating a fresh attacker-controlled wallet four times. Deterministic fixture, not observed data."),
        keeper_lag: BTreeMap::new(),
        lag_window_wallets: BTreeMap::new(),
        events,
    };
    s.sort_events();
    s.claim_count = s.claims();
    s
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayReport {
    pub impl_id: String,
    pub version: String,
    pub stream: String,
    pub claims: u64,
    pub paid: u64,
    pub rejected: u64,
    pub misreads: u64,
    pub duplicates: Vec<DuplicatePayout>,
    pub affected_wallets: BTreeSet<u32>,
    pub total_gross: i64,
    pub total_net: i64,
    pub total_fee: i64,
    pub diverted: i64,
    pub overpayment_gross: i64,
    pub per_period: Vec<PeriodStat>,
    pub per_wallet_paid: BTreeMap<u32, i64>,
    pub first_block: u64,
    pub last_block: u64,
    pub migrated_entries: u64,
}

impl ReplayReport {
    pub fn average_daily_gross(&self) -> i64 {
        if self.per_period.is_empty() {
            0
        } else {
            self.total_gross / self.per_period.len() as i64
        }
    }

    /// Overpayment as a share of average daily distribution over the periods
    /// in which duplicates occurred, in basis points.
    pub fn overpayment_share_bps(&self) -> i64 {
        let affected_periods: BTreeSet<u64> =
            self.duplicates.iter().map(|d| d.block_period).collect();
        if affected_periods.is_empty() || self.average_daily_gross() == 0 {
            return 0;
        }
        let per_day = self.overpayment_gross / affected_periods.len() as i64;
        per_day * 10_000 / self.average_daily_gross()
    }

    pub fn affected_period_range(&self) -> Option<(u64, u64)> {
        let min = self.duplicates.iter().map(|d| d.block_period).min()?;
        let max = self.duplicates.iter().map(|d| d.block_period).max()?;
        Some((min, max))
    }

    pub fn net_by_tier(&self, fee_bps: i64) -> [i64; 3] {
        let mut out = [0i64; 3];
        for (w, gross) in &self.per_wallet_paid {
            let t = (*w % 3) as usize;
            out[t] += gross - gross * fee_bps / 10_000;
        }
        out
    }

    pub fn wallets_by_tier(&self) -> [u64; 3] {
        let mut out = [0u64; 3];
        for w in self.per_wallet_paid.keys() {
            out[(*w % 3) as usize] += 1;
        }
        out
    }
}

/// Process every event of a stream against `state` in place, without
/// resetting accounting. Returns (claims, paid, rejected).
pub fn run_stream(state: &mut ProtocolState, stream: &ClaimStream) -> (u64, u64, u64) {
    let mut claims = 0u64;
    let mut paid = 0u64;
    let mut rejected = 0u64;
    for ev in &stream.events {
        match &ev.kind {
            ClaimKind::Rollover => state.rollover(ev.block),
            ClaimKind::Claim { wallet } => {
                claims += 1;
                match state.claim(*wallet, ev.block) {
                    Ok(ClaimOutcome::Paid { .. }) => paid += 1,
                    _ => rejected += 1,
                }
            }
            ClaimKind::Batch { wallets } => {
                claims += wallets.len() as u64;
                match state.claim_batch(wallets, ev.block) {
                    Ok(outs) => {
                        for o in outs {
                            if matches!(o, ClaimOutcome::Paid { .. }) {
                                paid += 1
                            } else {
                                rejected += 1
                            }
                        }
                    }
                    Err(_) => rejected += wallets.len() as u64,
                }
            }
        }
    }
    (claims, paid, rejected)
}

/// Shift every block of a stream by `offset` (to replay "future" periods on a live state).
pub fn shift_stream(mut stream: ClaimStream, offset: u64) -> ClaimStream {
    for e in &mut stream.events {
        e.block += offset;
    }
    stream.first_block += offset;
    stream.last_block += offset;
    stream.keeper_lag = stream
        .keeper_lag
        .iter()
        .map(|(p, l)| (p + offset / stream.period_length, *l))
        .collect();
    stream.lag_window_wallets = stream
        .lag_window_wallets
        .iter()
        .map(|(p, w)| (p + offset / stream.period_length, w.clone()))
        .collect();
    stream.name = format!("{}-from-{}", stream.name, stream.first_block);
    stream
}

/// Replay a stream against a copy of `state`. When `candidate` is given it is
/// installed on the copy first (running its migration, if any), so the replay
/// exercises the candidate's own functions. Returns the report and the
/// post-replay copy.
pub fn replay(
    state: &ProtocolState,
    stream: &ClaimStream,
    candidate: Option<&ImplementationBundle>,
) -> (ReplayReport, ProtocolState) {
    let mut s = state.clone();
    let mut migrated = 0;
    if let Some(b) = candidate {
        let r = s
            .install(b, "simulation", Commitment([0u8; 32]))
            .expect("fixture bundles are known to the local model");
        migrated = r.migrated_entries;
    }
    // Reset accounting so the report covers only this replay.
    s.distribution.per_period.clear();
    s.distribution.duplicates.clear();
    s.distribution.paid_in_period.clear();
    s.distribution.per_wallet_paid.clear();
    s.distribution.recent_payouts.clear();
    s.distribution.misreads = 0;
    s.distribution.fee_collected = 0;
    s.distribution.diverted = 0;
    s.distribution.payout_count = 0;
    let (claims, paid, rejected) = run_stream(&mut s, stream);
    let impl_id = s
        .distribution_package()
        .impl_for("claim(address)")
        .unwrap_or("")
        .to_string();
    let version = s.distribution_package().version.clone();
    let d = &s.distribution;
    let overpayment: i64 = d.duplicates.iter().map(|x| x.amount_gross).sum();
    let report = ReplayReport {
        impl_id,
        version,
        stream: stream.name.clone(),
        claims,
        paid,
        rejected,
        misreads: d.misreads,
        duplicates: d.duplicates.clone(),
        affected_wallets: d.duplicates.iter().map(|x| x.wallet).collect(),
        total_gross: d.per_period.values().map(|p| p.gross).sum(),
        total_net: d.per_period.values().map(|p| p.net).sum(),
        total_fee: d.fee_collected,
        diverted: d.diverted,
        overpayment_gross: overpayment,
        per_period: d.per_period.values().cloned().collect(),
        per_wallet_paid: d.per_wallet_paid.clone(),
        first_block: stream.first_block,
        last_block: stream.last_block,
        migrated_entries: migrated,
    };
    (report, s)
}

/// Wallets whose total paid amount differs between two replays.
pub fn differing_wallets(a: &ReplayReport, b: &ReplayReport) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for (w, v) in &a.per_wallet_paid {
        if b.per_wallet_paid.get(w) != Some(v) {
            out.insert(*w);
        }
    }
    for (w, v) in &b.per_wallet_paid {
        if a.per_wallet_paid.get(w) != Some(v) {
            out.insert(*w);
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuzzReport {
    pub property: String,
    pub version: String,
    pub runs: u32,
    pub claims: u64,
    pub violations: u64,
    pub runs_with_violation: u32,
    pub model: String,
}

/// Property: one payout per wallet per (block-derived) period. Random seeds,
/// random keeper lag, random repeat claims. This is a simulation on the local
/// model and establishes only what held for the cases run.
pub fn fuzz_payout_uniqueness(
    state: &ProtocolState,
    candidate: Option<&ImplementationBundle>,
    runs: u32,
    base_seed: u64,
) -> FuzzReport {
    let mut claims = 0u64;
    let mut violations = 0u64;
    let mut runs_with_violation = 0u32;
    let mut version = String::new();
    for r in 0..runs {
        let mut rng = Rng::new(base_seed ^ (r as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let periods = 2 + rng.below(4);
        let lag = rng.below(6);
        let lag_periods: Vec<u64> = (0..periods).filter(|_| rng.below(2) == 1).collect();
        let total = 80 + rng.below(240) as usize;
        let mut stream = build_stream(rng.next_u64(), periods, total, &lag_periods, lag, false);
        // Random wallets claiming inside a lag window and again after the rollover.
        if lag > 0 {
            for p in &lag_periods {
                let start = *p * stream.period_length;
                for _ in 0..(1 + rng.below(4)) {
                    let w = rng.below(2_000) as u32;
                    stream.events.push(ClaimEvent {
                        block: start + rng.below(lag),
                        kind: ClaimKind::Claim { wallet: w },
                    });
                    stream.events.push(ClaimEvent {
                        block: start + lag + 10 + rng.below(200),
                        kind: ClaimKind::Claim { wallet: w },
                    });
                }
            }
        }
        let extra = rng.below(40) as usize;
        for _ in 0..extra {
            let i = rng.below(stream.events.len() as u64) as usize;
            if let ClaimKind::Claim { wallet } = stream.events[i].kind {
                let block = (stream.events[i].block + 1 + rng.below(50)).min(stream.last_block);
                stream.events.push(ClaimEvent {
                    block,
                    kind: ClaimKind::Claim { wallet },
                });
            }
        }
        stream.sort_events();
        let (rep, _) = replay(state, &stream, candidate);
        version = rep.version.clone();
        claims += rep.claims;
        violations += rep.duplicates.len() as u64;
        if !rep.duplicates.is_empty() {
            runs_with_violation += 1;
        }
    }
    FuzzReport {
        property: "one payout per wallet per period (block-derived)".into(),
        version,
        runs,
        claims,
        violations,
        runs_with_violation,
        model:
            "random seeds, random keeper lag 0-5 blocks, random repeat claims; local replay model"
                .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{
        distribution_bundle_2_3_0, distribution_bundle_2_3_1, distribution_bundle_2_3_2, roles,
    };

    fn base_state() -> ProtocolState {
        ProtocolState::new(roles(), &distribution_bundle_2_3_0())
    }

    #[test]
    fn fixture_stream_has_expected_shape() {
        let s = distribution_fixture_stream();
        assert_eq!(s.claims(), FIXTURE_CLAIMS);
        assert_eq!(s.periods, FIXTURE_PERIODS);
        assert_eq!(
            s.lag_window_wallets.values().map(Vec::len).sum::<usize>(),
            FIXTURE_AFFECTED_WALLETS
        );
        assert_eq!(s, distribution_fixture_stream());
    }

    #[test]
    fn buggy_implementation_pays_fourteen_wallets_twice() {
        let s = distribution_fixture_stream();
        let (rep, _) = replay(&base_state(), &s, None);
        assert_eq!(
            rep.duplicates.len(),
            FIXTURE_AFFECTED_WALLETS,
            "{:?}",
            rep.duplicates
        );
        assert_eq!(rep.affected_wallets.len(), FIXTURE_AFFECTED_WALLETS);
        assert_eq!(rep.affected_period_range(), Some((27, 29)));
        let bps = rep.overpayment_share_bps();
        assert!((20..=40).contains(&bps), "overpayment share {bps} bps");
    }

    #[test]
    fn fixed_implementation_has_no_duplicates_and_differs_only_for_affected_wallets() {
        let s = distribution_fixture_stream();
        let (prod, _) = replay(&base_state(), &s, None);
        let (fixed, _) = replay(&base_state(), &s, Some(&distribution_bundle_2_3_1()));
        assert_eq!(fixed.duplicates.len(), 0);
        assert_eq!(fixed.claims, FIXTURE_CLAIMS as u64);
        assert_eq!(fixed.version, "2.3.1");
        let diff = differing_wallets(&prod, &fixed);
        assert_eq!(diff, prod.affected_wallets);
    }

    #[test]
    fn fuzz_property_holds_for_fix_and_fails_for_bug() {
        let st = base_state();
        let good = fuzz_payout_uniqueness(&st, Some(&distribution_bundle_2_3_1()), 24, 7);
        assert_eq!(good.violations, 0);
        let bad = fuzz_payout_uniqueness(&st, None, 24, 7);
        assert!(bad.violations > 0);
    }

    #[test]
    fn exploit_batches_drain_under_2_3_1_and_stop_under_2_3_2() {
        let st = ProtocolState::new(roles(), &distribution_bundle_2_3_1());
        let ex = exploit_stream(30_000, 12);
        let (before, _) = replay(&st, &ex, None);
        assert!(
            before.duplicates.len() >= 12 * 3,
            "{}",
            before.duplicates.len()
        );
        let (after, _) = replay(&st, &ex, Some(&distribution_bundle_2_3_2()));
        assert_eq!(after.duplicates.len(), 0);
    }
}
