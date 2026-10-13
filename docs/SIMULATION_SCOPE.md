# Simulation scope

Everything in this program runs on a deterministic local model. This file
states exactly what that model does, so that no screen can be read as a claim
about a real system.

## The protocol model (`crates/gup-domain/src/protocol.rs`)

* One package, `Distribution`, in a diamond-style system. Functions are
  addressed by selector (first four bytes of SHA-256 over the signature —
  illustrative, not Keccak).
* Behaviour is keyed by `impl_id` through a registry of known
  implementations (`protocol::impls`). Installing a payload swaps the
  `impl_id` behind a selector. Nothing is compiled or executed as bytecode.
* 2,000 eligible wallets; tier = `wallet % 3 + 1`; payout = tier × 1,000
  units; protocol fee `feeBps` (default 100 = 1.00 %).
* Periods are 1,000 blocks. The keeper's `rollover()` transaction advances
  `keeperPeriod`.
* Duplicate detection: a wallet paid twice for one block-derived period.
* No chain, no fork, no gas, no reorgs, no concurrency, no external calls.

### Implementations in the registry

| impl id | behaviour |
| --- | --- |
| `distribution.claim/2.3.0` | eligibility keyed on `keeperPeriod` (the rollover bug) |
| `distribution.claim/2.3.1` | eligibility keyed on the block-derived period (the fix) |
| `distribution.claim/2.3.1-flat-amount` | the fix, but tier weight dropped (failure mode 1) |
| `distribution.claim/2.3.1-diverted` | the fix, but the fee is rerouted (substitution attack payload) |
| `distribution.claim/3.0.0` | packed `ClaimRecord` storage, format 2 (case C) |
| `distribution.claim/3.0.1-reverse` | format-1 logic after a reverse migration (recovery proposal) |
| `distribution.claimFor/2.3.1` | batch claim with the in-batch duplicate fault (case D) |
| `distribution.claimFor/2.3.2` | batch claim, fixed |
| `distribution.rollover/1` | keeper rollover |

A payload naming any other implementation is rejected by the model.

## Case A — Distribution 2.3.0 → 2.3.1

Fixture stream (`simulation::distribution_fixture_stream`): 41,200 claim
transactions over 30 periods, seed `0x5eedd15d41b00001`. On periods 27, 28
and 29 the keeper rollover lands three blocks late. Fourteen chosen wallets
(7 tier-1, 3 tier-2, 4 tier-3, picked so the overpayment lands near 0.3 % of
daily distribution) claim inside that window and again after rollover; under
2.3.0 each is paid twice for one period. These figures reproduce the
hypothetical example on page 2 of the review draft and are **illustrative
fixtures**.

What the program actually does: replays the stream on a copy of the genesis
state with 2.3.0 (production) and with 2.3.1 installed (candidate), compares
per-wallet totals, runs a property fuzz (16 runs, random seeds, random keeper
lag 0–5 blocks, random repeat claims and lag-window pairs). Reported: counts,
duplicates, overpayment in units and as basis points of average daily gross
over the affected periods.

What it does not do: fork any chain, replay real transactions, verify real
bytecode, check the meaning of `lastPaidPeriod` (asserted), re-verify the
keeper (assumed).

## Case B — feeBps 100 → 250

Same stream, replayed with the two fee values on genesis at 2.3.1. The
economic comparison is arithmetic on the two replays: net payouts by tier and
treasury fee income. User behaviour does not respond to the fee in the model.

## Case C — Distribution 2.3.1 → 3.0.0 (migration)

Live state: 30 periods of history under 2.3.1 (~2,000 wallets with
`lastPaidPeriod`). The candidate's migration packs each entry into a
`ClaimRecord`. The dry run installs the candidate on a copy of the live state
(running the migration) and replays a 3-period synthetic stream (4,000
claims) starting at block 30,000.

Misreads: when format-1 code (2.3.x) reads a format-2 state, the model
counts a misread and refuses the claim with a garbled-value explanation. The
recovery assessment probes up to 200 migrated wallets with the prior reader
to demonstrate layer 2.

## Case D — active incident

Live state: 30 periods under 2.3.1, then 12 attacker batches (one per block
from block 30,000), each repeating a fresh attacker-controlled wallet four
times, so `claimFor/2.3.1` pays three duplicates per block. The candidate
2.3.2 is replayed against the same batches.

Incident policy comparison (`incident::simulate`): a closed-form timeline per
policy — detection at block 3, candidate fix at block 4, quorum at block 60
(parameter), containment at detection+1 if the policy pre-authorizes a
state-changing rung, loss per block while uncontained, legitimate activity
interrupted per block while contained, containment lapsing at the policy's
maximum duration, lifting only at quorum by guardians. The false-positive
switch zeroes the exploit to show the cost of premature action. All numbers
are parameters.

## Fuzzing

`simulation::fuzz_payout_uniqueness`: random seeds, random period counts
(2–5), random keeper lag, random lag-window pairs, random repeat claims. It
establishes only that the property held for the cases generated.

## Persistence

The workspace is serialized to `workspace.json` in the app data directory;
claim streams and genesis states are rebuilt from fixtures on load. Audit
events are appended to `audit.jsonl`.
