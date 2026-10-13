//! Canonical serialization and the artifact commitment `H`.
//!
//! The commitment is what guardians approve and what the executor is allowed
//! to apply. It must be reproducible from the actual package contents by
//! anyone, with no input from the proposing agent's displayed summary.
//!
//! Definition (documented in `docs/VERIFICATION.md`):
//!
//! ```text
//! manifest_canonical = canonical JSON of the manifest
//!                      (object keys sorted bytewise, no whitespace,
//!                       integers only, UTF-8, JSON string escaping)
//! preimage = b"GUP-ARTIFACT-COMMITMENT/v1\n"
//!          || u64_be(len(manifest_canonical)) || manifest_canonical
//!          || u64_be(len(payload))            || payload
//! H = SHA-256(preimage)
//! ```
//!
//! `gup-verify preimage` writes the exact preimage bytes so that `sha256sum`
//! or any other tool reproduces `H` without this code base.

use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;

/// Domain separation prefix for the artifact commitment.
pub const COMMITMENT_DOMAIN: &[u8] = b"GUP-ARTIFACT-COMMITMENT/v1\n";

#[derive(Debug, thiserror::Error)]
pub enum CanonicalError {
    #[error("canonical form allows integers only; found non-integer number {0}")]
    NonInteger(String),
    #[error("value could not be converted to JSON: {0}")]
    Json(String),
}

/// Serialize any serde value into canonical JSON bytes.
pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CanonicalError> {
    let v = serde_json::to_value(value).map_err(|e| CanonicalError::Json(e.to_string()))?;
    canonical_json(&v)
}

/// Canonical JSON: sorted object keys (bytewise), no whitespace, integers only.
pub fn canonical_json(value: &Value) -> Result<Vec<u8>, CanonicalError> {
    let mut out = Vec::with_capacity(256);
    write_value(value, &mut out)?;
    Ok(out)
}

fn write_value(v: &Value, out: &mut Vec<u8>) -> Result<(), CanonicalError> {
    match v {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                out.extend_from_slice(i.to_string().as_bytes());
            } else if let Some(u) = n.as_u64() {
                out.extend_from_slice(u.to_string().as_bytes());
            } else {
                return Err(CanonicalError::NonInteger(n.to_string()));
            }
        }
        Value::String(s) => {
            let escaped = serde_json::to_string(s).expect("string serialization cannot fail");
            out.extend_from_slice(escaped.as_bytes());
        }
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_value(item, out)?;
            }
            out.push(b']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push(b'{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                let ek = serde_json::to_string(k).expect("string serialization cannot fail");
                out.extend_from_slice(ek.as_bytes());
                out.push(b':');
                write_value(&map[*k], out)?;
            }
            out.push(b'}');
        }
    }
    Ok(())
}

/// Plain SHA-256.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(sha256(bytes))
}

/// The artifact commitment `H`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Commitment(pub [u8; 32]);

impl Commitment {
    /// `0x` followed by 64 lowercase hex characters.
    pub fn hex(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }

    /// Display form in the style of the review draft: `0x7c1e…a94d`.
    pub fn short(&self) -> String {
        let h = hex::encode(self.0);
        format!("0x{}…{}", &h[..4], &h[h.len() - 4..])
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let t = s.trim();
        let t = t
            .strip_prefix("0x")
            .or_else(|| t.strip_prefix("0X"))
            .unwrap_or(t);
        let bytes = hex::decode(t).map_err(|e| format!("invalid hex: {e}"))?;
        if bytes.len() != 32 {
            return Err(format!("expected 32 bytes, found {}", bytes.len()));
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes);
        Ok(Commitment(out))
    }
}

impl fmt::Debug for Commitment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Commitment({})", self.hex())
    }
}

impl fmt::Display for Commitment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

impl Serialize for Commitment {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for Commitment {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Commitment::parse(&s).map_err(D::Error::custom)
    }
}

/// The exact bytes hashed to produce `H`.
pub fn commitment_preimage(manifest_canonical: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut pre =
        Vec::with_capacity(COMMITMENT_DOMAIN.len() + 16 + manifest_canonical.len() + payload.len());
    pre.extend_from_slice(COMMITMENT_DOMAIN);
    pre.extend_from_slice(&(manifest_canonical.len() as u64).to_be_bytes());
    pre.extend_from_slice(manifest_canonical);
    pre.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    pre.extend_from_slice(payload);
    pre
}

/// Compute `H` from canonical manifest bytes and the raw payload bytes.
pub fn compute_commitment(manifest_canonical: &[u8], payload: &[u8]) -> Commitment {
    Commitment(sha256(&commitment_preimage(manifest_canonical, payload)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_sorts_keys_and_strips_whitespace() {
        let v = json!({"b": 1, "a": {"z": [1, 2, {"y": "x"}], "c": true}, "s": "qu\"ote"});
        let bytes = canonical_json(&v).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r#"{"a":{"c":true,"z":[1,2,{"y":"x"}]},"b":1,"s":"qu\"ote"}"#
        );
    }

    #[test]
    fn canonical_rejects_floats() {
        let v = json!({"x": 1.5});
        assert!(matches!(
            canonical_json(&v),
            Err(CanonicalError::NonInteger(_))
        ));
    }

    #[test]
    fn canonical_is_order_independent() {
        let a = json!({"k1": 1, "k2": {"n": 2, "m": 3}});
        let b = json!({"k2": {"m": 3, "n": 2}, "k1": 1});
        assert_eq!(canonical_json(&a).unwrap(), canonical_json(&b).unwrap());
    }

    #[test]
    fn commitment_is_deterministic_and_sensitive() {
        let m = b"{\"a\":1}";
        let p = b"payload";
        let h1 = compute_commitment(m, p);
        let h2 = compute_commitment(m, p);
        assert_eq!(h1, h2);
        assert_ne!(h1, compute_commitment(m, b"payloae"));
        assert_ne!(h1, compute_commitment(b"{\"a\":2}", p));
        // Length framing prevents boundary ambiguity.
        assert_ne!(
            compute_commitment(b"ab", b"c"),
            compute_commitment(b"a", b"bc")
        );
    }

    #[test]
    fn commitment_matches_manual_sha256_of_preimage() {
        let m = b"{\"a\":1}";
        let p = b"payload";
        let pre = commitment_preimage(m, p);
        assert_eq!(compute_commitment(m, p).0, sha256(&pre));
        assert!(pre.starts_with(COMMITMENT_DOMAIN));
    }

    #[test]
    fn commitment_hex_roundtrip() {
        let h = compute_commitment(b"m", b"p");
        let s = h.hex();
        assert!(s.starts_with("0x") && s.len() == 66);
        assert_eq!(Commitment::parse(&s).unwrap(), h);
        assert_eq!(h.short().len(), "0x1234…5678".len());
        let json = serde_json::to_string(&h).unwrap();
        let back: Commitment = serde_json::from_str(&json).unwrap();
        assert_eq!(back, h);
    }
}
