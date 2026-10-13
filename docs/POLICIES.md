# Policies: illustrative configurations, not endorsed standards

The review draft establishes an authority split and a set of open questions.
It does not establish a quorum size, a timelock, an incident-pause authority,
an automatic rollback, a definition of "low-risk" or who classifies a change.
Every configurable policy in this program is therefore an **example with
stated assumptions**, selectable for comparison, with normal approval as the
default.

## Quorum presets (`guardians::quorum_presets`)

| id | m-of-n | status | assumptions |
| --- | --- | --- | --- |
| `demo-2-of-3` | 2 of 3 | demonstration default | three independent keys; any two reachable in the decision window |
| `unanimous-3-of-3` | 3 of 3 | alternative | all guardians reachable |
| `lighter-1-of-3` | 1 of 3 | hypothetical | one guardian's judgment suffices for the change class — explores case B's open question; not endorsed |
| `wide-3-of-5` | 3 of 5 | alternative | five independent keys (the demo generates five local test keys) |
| custom | m of n (1 ≤ m ≤ n ≤ 9) | reviewer-defined | every key independently controlled |

Changing the guardian count invalidates signatures on file; the engine
re-evaluates quorum against the new policy.

## Incident policies (`incident::incident_policies`)

| id | pre-authorized rung | bound | lift |
| --- | --- | --- | --- |
| `normal-approval` (default) | none | — | — |
| `pre-flag` | flag only | no state change | — |
| `pre-draft` | flag and draft only | no state change | — |
| `pre-limit-function` | limit one function (`claimFor`) | 200 blocks | guardians only |
| `pre-pause-package` | pause the Distribution package | 200 blocks | guardians only |
| `pre-pause-protocol` | pause everything | 200 blocks | guardians only |

Flagging an incident and drafting a fix change no protocol state; they are the agent's ordinary work (the same as Stage A) and are granted under every policy. All policies are flagged `hypothetical: true`. Under any policy the engine refuses a
request above the pre-authorized rung, refuses the agent lifting its own
containment, expires the grant at its bound, and voids every grant when the
incident is closed. Whether the agent that drafts the fix should hold any of
these powers is question 5 for the reviewer.

## What replaced the over-specified choices of the earlier implementation prompt

| earlier choice | now |
| --- | --- |
| fixed 2-of-3 quorum | configurable m-of-n; 2-of-3 is a labelled preset |
| mandatory timelock | none modelled; quorum arrival time is a scenario parameter |
| fixed incident-pause authority | selectable hypothetical policies; normal approval default |
| automatically permitted rollback | four-layer recovery model; automatic recovery blocked when layers 2–4 require more; recovery is a distinct proposal |
| a single global definition of low-risk | none; the draft's open question ("who classifies an upgrade as low-risk?") is shown instead |
| agent-controlled risk classification | removed; the proposer never sets its own approval requirements or scrutiny level |
| "Rollback successful" | replaced by the recovery assessment and the code-revert attempt that is blocked or drafted as a new proposal |

## Candidate and substitution variants (for failure modes)

* Candidate variant `flat_amount` (case A): passes the checked properties, pays the wrong amount — failure mode 1.
* Substitutions after review: `divert_fee`, `reroute_selector`, `silent_parameter`, `manifest_and_payload` — failure mode 5. Each leaves the agent's displayed summary untouched; each is refused by the executor and rejected by the verifier.
