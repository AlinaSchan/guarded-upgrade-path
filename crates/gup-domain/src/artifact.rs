//! Artifact model: a versioned manifest plus an executable payload.
//!
//! The payload is **deterministic example data**, not EVM bytecode. It is an
//! "implementation bundle": a canonical JSON description of the functions a
//! package exposes, each bound to an `impl_id` that the local protocol model
//! knows how to interpret, plus the declared storage layout, parameters,
//! roles and an optional migration. The executor applies the payload it is
//! handed, never the manifest and never the agent's displayed summary.

use crate::canonical::{
    canonical_bytes, compute_commitment, sha256_hex, CanonicalError, Commitment,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MANIFEST_SCHEMA: &str = "gup-manifest/1";
pub const BUNDLE_FORMAT: &str = "gup-implbundle/1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Case A: one function body changes; interface, storage and roles unchanged.
    ImplementationOnly,
    /// Case B: implementation unchanged; one parameter value changes.
    ParameterOnly,
    /// Case C: implementation changes and persistent state is transformed.
    StateMigration,
    /// Case D: a fix prepared while a fault is being exploited.
    IncidentFix,
    /// A recovery proposal (e.g. a reverse migration) raised as a new decision.
    Recovery,
}

impl ChangeKind {
    pub fn label(&self) -> &'static str {
        match self {
            ChangeKind::ImplementationOnly => "implementation change, state unchanged",
            ChangeKind::ParameterOnly => "parameter-only change",
            ChangeKind::StateMigration => "state migration",
            ChangeKind::IncidentFix => "incident fix",
            ChangeKind::Recovery => "recovery proposal",
        }
    }
}

/// A function as declared in the manifest (post-upgrade interface).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionEntry {
    pub selector: String,
    pub name: String,
    pub impl_id: String,
    pub privileged: bool,
}

/// A function as carried in the payload, including its (pseudo-source) body
/// and the storage it reads and writes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionImpl {
    pub selector: String,
    pub name: String,
    pub impl_id: String,
    pub privileged: bool,
    pub body: String,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
}

impl FunctionImpl {
    pub fn entry(&self) -> FunctionEntry {
        FunctionEntry {
            selector: self.selector.clone(),
            name: self.name.clone(),
            impl_id: self.impl_id.clone(),
            privileged: self.privileged,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageSlot {
    pub index: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageLayout {
    pub namespace: String,
    /// Format version of the persistent state this layout describes.
    pub format_version: u32,
    pub slots: Vec<StorageSlot>,
}

impl StorageLayout {
    pub fn slot_names(&self) -> Vec<String> {
        self.slots
            .iter()
            .map(|s| format!("{}.{}", self.namespace, s.name))
            .collect()
    }
    pub fn slot(&self, name: &str) -> Option<&StorageSlot> {
        self.slots.iter().find(|s| s.name == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RoleSet {
    pub upgrade_admin: Vec<String>,
    pub guardians: Vec<String>,
}

impl RoleSet {
    pub fn normalized(mut self) -> Self {
        self.upgrade_admin.sort();
        self.upgrade_admin.dedup();
        self.guardians.sort();
        self.guardians.dedup();
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationSpec {
    pub id: String,
    pub description: String,
    pub from_format: u32,
    pub to_format: u32,
    /// What the proposer asserts about reverting the code afterwards. This is
    /// an assertion carried in the package; the recovery model checks it.
    pub proposer_reversibility_claim: String,
}

/// The versioned manifest. Everything in here is also derivable from the
/// payload; the binding check (`payload.binds_manifest`) verifies that.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub schema: String,
    pub package: String,
    pub from_version: String,
    pub to_version: String,
    pub change_kind: ChangeKind,
    pub summary: String,
    pub functions: Vec<FunctionEntry>,
    pub storage: StorageLayout,
    pub parameters: BTreeMap<String, i64>,
    pub roles: RoleSet,
    pub migration: Option<MigrationSpec>,
    /// Hex SHA-256 of the payload bytes.
    pub payload_sha256: String,
    pub produced_by: String,
}

/// The payload: an implementation bundle serialized canonically.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationBundle {
    pub format: String,
    pub package: String,
    pub version: String,
    pub functions: Vec<FunctionImpl>,
    pub storage: StorageLayout,
    pub parameters: BTreeMap<String, i64>,
    pub roles: RoleSet,
    pub migration: Option<MigrationSpec>,
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("payload is not a valid implementation bundle: {0}")]
    Unparseable(String),
    #[error("payload format {0} is not supported (expected {BUNDLE_FORMAT})")]
    UnsupportedFormat(String),
    #[error("canonicalization failed: {0}")]
    Canonical(#[from] CanonicalError),
}

impl ImplementationBundle {
    pub fn payload_bytes(&self) -> Result<Vec<u8>, ArtifactError> {
        Ok(canonical_bytes(self)?)
    }

    pub fn from_payload_bytes(bytes: &[u8]) -> Result<Self, ArtifactError> {
        let b: ImplementationBundle =
            serde_json::from_slice(bytes).map_err(|e| ArtifactError::Unparseable(e.to_string()))?;
        if b.format != BUNDLE_FORMAT {
            return Err(ArtifactError::UnsupportedFormat(b.format));
        }
        Ok(b)
    }

    pub fn function(&self, name: &str) -> Option<&FunctionImpl> {
        self.functions.iter().find(|f| f.name == name)
    }

    pub fn function_mut(&mut self, name: &str) -> Option<&mut FunctionImpl> {
        self.functions.iter_mut().find(|f| f.name == name)
    }

    pub fn entries(&self) -> Vec<FunctionEntry> {
        self.functions.iter().map(FunctionImpl::entry).collect()
    }
}

/// Derive a 4-byte selector from a function signature.
///
/// Illustrative: the first four bytes of SHA-256 over the signature. Real
/// EVM systems use Keccak-256; the local model only needs a stable,
/// collision-free mapping.
pub fn selector_for(signature: &str) -> String {
    let h = crate::canonical::sha256(signature.as_bytes());
    format!("0x{}", hex::encode(&h[..4]))
}

/// Manifest plus payload bytes. The commitment is computed from these two
/// values and nothing else.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub manifest: ArtifactManifest,
    #[serde(with = "payload_hex")]
    pub payload: Vec<u8>,
}

mod payload_hex {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        hex::encode(bytes).serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        hex::decode(s).map_err(serde::de::Error::custom)
    }
}

impl Artifact {
    /// Build an artifact from a bundle. The manifest is derived from the
    /// bundle so that, at construction time, summary and payload agree.
    pub fn from_bundle(
        bundle: &ImplementationBundle,
        from_version: &str,
        change_kind: ChangeKind,
        summary: &str,
        produced_by: &str,
    ) -> Result<Self, ArtifactError> {
        let payload = bundle.payload_bytes()?;
        let manifest = ArtifactManifest {
            schema: MANIFEST_SCHEMA.to_string(),
            package: bundle.package.clone(),
            from_version: from_version.to_string(),
            to_version: bundle.version.clone(),
            change_kind,
            summary: summary.to_string(),
            functions: bundle.entries(),
            storage: bundle.storage.clone(),
            parameters: bundle.parameters.clone(),
            roles: bundle.roles.clone().normalized(),
            migration: bundle.migration.clone(),
            payload_sha256: sha256_hex(&payload),
            produced_by: produced_by.to_string(),
        };
        Ok(Artifact { manifest, payload })
    }

    pub fn manifest_canonical(&self) -> Result<Vec<u8>, CanonicalError> {
        canonical_bytes(&self.manifest)
    }

    pub fn manifest_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.manifest).unwrap_or_default()
    }

    /// Recompute `H` from the manifest and payload actually held.
    pub fn commitment(&self) -> Result<Commitment, CanonicalError> {
        Ok(compute_commitment(
            &self.manifest_canonical()?,
            &self.payload,
        ))
    }

    pub fn bundle(&self) -> Result<ImplementationBundle, ArtifactError> {
        ImplementationBundle::from_payload_bytes(&self.payload)
    }

    pub fn payload_sha256(&self) -> String {
        sha256_hex(&self.payload)
    }

    pub fn payload_utf8(&self) -> String {
        String::from_utf8_lossy(&self.payload).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> ImplementationBundle {
        ImplementationBundle {
            format: BUNDLE_FORMAT.into(),
            package: "Distribution".into(),
            version: "2.3.1".into(),
            functions: vec![FunctionImpl {
                selector: selector_for("claim(address)"),
                name: "claim(address)".into(),
                impl_id: "distribution.claim/2.3.1".into(),
                privileged: false,
                body: "...".into(),
                reads: vec!["Distribution.lastPaidPeriod".into()],
                writes: vec!["Distribution.lastPaidPeriod".into()],
            }],
            storage: StorageLayout {
                namespace: "Distribution".into(),
                format_version: 1,
                slots: vec![StorageSlot {
                    index: 0,
                    name: "lastPaidPeriod".into(),
                    ty: "mapping(address=>uint64)".into(),
                }],
            },
            parameters: BTreeMap::from([("feeBps".to_string(), 100)]),
            roles: RoleSet {
                upgrade_admin: vec!["admin".into()],
                guardians: vec!["g1".into()],
            },
            migration: None,
        }
    }

    #[test]
    fn payload_roundtrip_and_commitment_reproducible() {
        let b = bundle();
        let a = Artifact::from_bundle(&b, "2.3.0", ChangeKind::ImplementationOnly, "fix", "agent")
            .unwrap();
        let parsed = a.bundle().unwrap();
        assert_eq!(parsed, b);
        let h1 = a.commitment().unwrap();
        let a2 = Artifact::from_bundle(&b, "2.3.0", ChangeKind::ImplementationOnly, "fix", "agent")
            .unwrap();
        assert_eq!(h1, a2.commitment().unwrap());
        assert_eq!(a.manifest.payload_sha256, a.payload_sha256());
        let json = serde_json::to_string(&a).unwrap();
        let back: Artifact = serde_json::from_str(&json).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn changing_payload_changes_commitment() {
        let b = bundle();
        let a = Artifact::from_bundle(&b, "2.3.0", ChangeKind::ImplementationOnly, "fix", "agent")
            .unwrap();
        let mut tampered = a.clone();
        let mut tb = b.clone();
        tb.functions[0].impl_id = "distribution.claim/2.3.1-diverted".into();
        tampered.payload = tb.payload_bytes().unwrap();
        assert_ne!(a.commitment().unwrap(), tampered.commitment().unwrap());
    }

    #[test]
    fn selector_is_stable() {
        assert_eq!(
            selector_for("claim(address)"),
            selector_for("claim(address)")
        );
        assert_ne!(
            selector_for("claim(address)"),
            selector_for("claimFor(address[])")
        );
        assert!(selector_for("claim(address)").starts_with("0x"));
        assert_eq!(selector_for("claim(address)").len(), 10);
    }
}
