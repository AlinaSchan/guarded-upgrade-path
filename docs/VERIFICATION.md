# Verification: what is cryptographically verified and what is simulated

This document defines the artifact commitment `H`, the approval signature
format, the verifier's inputs, and the exact line between genuine
verification and simulation in this demo.

## 1. Artifact commitment `H`

An artifact is a **manifest** (`gup-manifest/1`, JSON) plus a **payload**
(raw bytes). In this demo the payload is a canonical-JSON implementation
bundle (`gup-implbundle/1`) that stands in for bytecode; see
`SIMULATION_SCOPE.md`.

### Canonical serialization of the manifest

* JSON object keys sorted bytewise (UTF-8), no whitespace.
* Integers only; any non-integer number is rejected.
* Strings escaped with standard JSON escaping (as `serde_json` emits).
* Arrays keep their order.

Implementation: `crates/gup-domain/src/canonical.rs` (`canonical_json`).

### Preimage and hash

```text
manifest_canonical = canonical JSON bytes of the manifest
preimage = b"GUP-ARTIFACT-COMMITMENT/v1\n"
         || u64_be(len(manifest_canonical)) || manifest_canonical
         || u64_be(len(payload))            || payload
H = SHA-256(preimage)
```

Length framing removes boundary ambiguity; the domain prefix separates this
hash from any other SHA-256 use. `H` is displayed as `0x` + 64 hex
characters, or shortened as `0x7c1e…a94d` in the style of the review draft.

The manifest also carries `payload_sha256`; the binding check
(`checks::manifest_binding_check`) verifies it together with every declared
field against the parsed payload. `H` covers the payload bytes directly, so
the binding check is a *description* check, not part of the commitment.

### Reproducing `H` without this code base

```bash
gup-verify preimage --manifest manifest.json --payload payload.bin --out preimage.bin
sha256sum preimage.bin        # equals H
```

`gup-verify hash` prints `H` directly. Both read only the two files.

## 2. Guardian approvals

Each guardian signs an Ed25519 signature over:

```text
"GUP-APPROVAL/v1" "\n" proposal_id "\n" revision "\n" decision "\n" H_hex
```

where `decision` is `approve`, `reject` or `request_changes` and `H_hex` is
the commitment the guardian *recomputed* (the engine signs
`artifact.commitment()`, never the agent's displayed value).

Quorum evaluation (`guardians::evaluate_quorum`) counts an approval only if:

* the signature verifies against the guardian's public key,
* the decision is `approve`,
* the signed commitment equals the commitment being authorized,
* the proposal id and revision match,
* the guardian has not already been counted.

A changed proposal increments the revision; earlier signatures stop
counting. The resulting `Authorization` carries the policy, the counted
approvals and `H`.

### Keys

Guardian keys are **local test keys** derived deterministically from public
seeds (`SHA-256("gup-local-test-key/<i>")`). The demo holds every private
key, so clicking "approve exactly H" as Guardian 2 is not authentication of
anyone. Signature *verification* is genuine (`ed25519-dalek`); guardian
*identity* is simulated. The exported `guardians.json` contains public keys
only.

## 3. The verifier

`gup_domain::verifier::verify(manifest_json, payload, expected, authorization, guardians)`
is used by:

* the executor before applying anything (`executor::execute`),
* the in-app "run verifier in-process" button (`Workspace::verify_independently`),
* the `gup-verify` CLI, out of process, on files the reviewer controls.

Inputs are only: manifest bytes, payload bytes, the expected `H` (from the
authorization or supplied), the authorization (signatures) and the guardian
public keys. The agent's displayed summary and claimed hash are never
inputs; the exported bundle includes `displayed_summary.txt` purely so the
reviewer can see what the interface showed.

The verifier reports, per step: parse manifest, recompute `H`, manifest
describes payload, compare with expected `H`, verify signatures, quorum. The
verdict is `Accept` only if every step passes.

### CLI exit codes

| code | meaning |
| --- | --- |
| 0 | verified |
| 2 | mismatch, invalid signature, quorum not met, or manifest/payload disagreement |
| 1 | usage or I/O error |

## 4. What the executor does

1. Checks the authorization refers to this proposal.
2. Recomputes `H` over the manifest and payload it was handed; refuses on mismatch (`CommitmentMismatch`).
3. Verifies signatures and quorum over the recomputed `H`; refuses otherwise.
4. Parses the payload and installs it in the local model (running the declared migration, if any). It never edits the payload.
5. Verifies the post-state (installed bundle equals payload, state format matches the layout, roles unchanged, structural checks, a short smoke replay). An unexpected post-state is reported as an escalation: a new guardian decision, not a rollback.

## 5. Summary table

| Claim | Genuine | Simulated |
| --- | --- | --- |
| `H` is reproducible from manifest + payload | yes | — |
| Approvals are Ed25519 signatures over `H` | yes | — |
| Signatures come from independent guardians | — | yes (local test keys) |
| Manifest describes the payload | yes | — |
| Structural checks on the payload | yes (on the demo's artifact format) | — |
| The payload is bytecode that runs on a chain | — | yes (interpreted implementation ids) |
| Replay results, fuzz results, economic figures | — | yes (local model) |
| The executor applies only the authorized `H` | yes (enforced in Rust) | — |
| Protocol state, pauses, incidents, timelines | — | yes |
