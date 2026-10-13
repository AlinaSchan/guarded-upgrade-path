//! Guarded Upgrade Path — domain engine.
//!
//! An interactive technical review sandbox for exploring the boundary between
//! autonomous upgrade preparation and human protocol authority. The working
//! hypothesis under test: an agent can observe, prepare a candidate, verify
//! selected properties, simulate and assemble evidence; the decision to change
//! the protocol stays with designated human guardians; the executor applies
//! only the exact artifact the guardians approved. The engine is built to show
//! where that separation can fail, not to present it as settled.
//!
//! Everything in this crate runs offline on a deterministic local model. See
//! `docs/SIMULATION_SCOPE.md` for what is simulated and `docs/VERIFICATION.md`
//! for what is cryptographically verified.

pub mod artifact;
pub mod canonical;
pub mod checks;
pub mod engine;
pub mod evidence;
pub mod executor;
pub mod exports;
pub mod failure_modes;
pub mod fixtures;
pub mod guardians;
pub mod incident;
pub mod notes;
pub mod protocol;
pub mod questions;
pub mod recovery;
pub mod scenarios;
pub mod simulation;
pub mod surface;
pub mod verifier;
pub mod views;

pub use canonical::{compute_commitment, Commitment};
pub use engine::{AgentStep, EngineError, ProposalStatus, Workspace};

/// Crate version, surfaced in the UI and exports.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Plain statement of what the demo does and does not establish.
pub fn scope_statement() -> &'static str {
    "This desktop program is a review aid built around the Guarded Upgrade Path review draft. \
It runs a deterministic local model of one package in a diamond-style system. It does not connect to any chain, \
fork any network, execute bytecode, hold production keys or participate in any live governance. Hash recomputation \
and Ed25519 signature verification are genuine; guardian identity, the protocol, the replay and every figure are simulated. \
The hypothesis it explores is not presented as an established or universally safe governance model."
}
