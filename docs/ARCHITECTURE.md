# Architecture

```
guarded-upgrade-path/
├── crates/gup-domain      Rust library: the whole domain model and engine (no UI, no I/O beyond serde)
├── crates/gup-verify      Rust binary: out-of-process verifier CLI (reads files only)
├── src-tauri              Tauri 2 desktop shell: commands, persistence, exports
├── src                    React + TypeScript frontend (renders views; decides nothing)
├── docs                   this documentation
└── .github/workflows      CI (tests, lint, packaging smoke) and draft-release packaging
```

## Domain crate (`gup-domain`)

| module | responsibility |
| --- | --- |
| `canonical` | canonical JSON, SHA-256, the commitment `H` and its preimage |
| `artifact` | manifest, implementation bundle (payload), `Artifact` |
| `fixtures` | deterministic example bundles for every version and variant |
| `protocol` | the local protocol model: packages, storage, roles, pause state, Distribution state, claim behaviours, migrations, install |
| `simulation` | seeded claim streams, replay, fuzzing, exploit stream |
| `checks` | deterministic structural checks and the manifest–payload binding check |
| `evidence` | four classifications, evidence items, the evidence package, technical verdict |
| `surface` | approval surface projections (minimal / structured / full) and comparison |
| `guardians` | local test guardians, quorum policies, Ed25519 approvals, quorum evaluation |
| `verifier` | independent verification report (shared by executor, app and CLI) |
| `executor` | constrained execution: verify, apply only `H`, post-state verification, escalation |
| `recovery` | four-layer recovery assessment; blocked automatic revert; recovery proposals |
| `incident` | authority ladder, hypothetical policies, timeline simulation, bounded grants |
| `scenarios` | the four cases: metadata, genesis/live states, each agent step's content, evidence assembly, substitutions |
| `engine` | `ScenarioInstance`, `ProposalStatus` state machine, `Workspace`, audit log, every action |
| `failure_modes` | the six failure modes as executable regression scenarios |
| `questions`, `notes`, `exports`, `views` | the five questions, review notes, Markdown exports, UI view models |

### Proposal state machine (`engine::ProposalStatus`)

```
Stage A (agent)      idle → observed → detected → diagnosed → constructed → checked → simulated → assembled → committed → submitted
Stage B (guardians)  submitted → under_review → { approved | rejected | changes_requested }
                     changes_requested → (revise: revision+1, approvals invalidated) → diagnosed → …
Stage C (executor)   approved → { executed | execution_refused | escalated }
Recovery             executed/escalated → (users act) → assess / attempt code revert (blocked or drafted) / draft recovery proposal → new proposal id → constructed → …
```

Every transition is validated in Rust; invalid requests return
`EngineError::InvalidTransition`. The frontend only calls commands.

### Authority boundaries, where enforced

* The agent cannot authorize: no agent step produces an `Authorization`; `execute` requires one.
* Guardians sign the recomputed `H` (`artifact.commitment()`), not the displayed value.
* The executor recomputes `H`, verifies signatures and quorum (`verifier::verify`), and installs the payload it was handed.
* Emergency grants: `incident::agent_request_action` is the only path; it is bounded by policy, incident and expiry; the agent cannot lift.
* Recovery: `recovery::attempt_code_revert` returns `Blocked` when layer 2 or 4 requires more; recovery is a new proposal.

## Desktop shell (`src-tauri`)

`main.rs` is the entry point: on Windows and macOS it starts `desktop.rs`;
on every other target it compiles only `unsupported.rs`, a stub that prints
an error and exits (the Tauri dependencies are target-specific in
`Cargo.toml`, so nothing else is built there).
`desktop.rs` holds `AppState { Mutex<Workspace>, data_dir }`. Every mutating
command runs through `mutate`, which persists `workspace.json` (atomic
rename) and appends new audit events to `audit.jsonl`. Timestamps are added
at persistence time. Exports write Markdown or a verification bundle to paths
chosen through the native dialog plugin. No network access anywhere.

## Frontend (`src`)

`App.tsx` holds the workspace view, per-case scenario views and the route.
Pages: Overview, Scenario (Stage A), Approval (Stage B), Execution (Stage C),
Recovery, Incident, Failure modes, Review Lab, Questions, Notes, Audit, Scope.
`types.ts` mirrors the Rust view structs; `api.ts` wraps `invoke`. Outside
the desktop shell (a plain browser tab) `main.tsx` renders only a notice
(`components/DesktopOnly.tsx`); there is no web version.

## Testing

* `cargo test -p gup-domain`: canonical form, artifacts, fixtures, simulation, guardians, incident, questions, engine state machine, the six failure modes.
* `cargo test -p gup-verify`: the CLI, out of process, on an exported bundle and on substituted artifacts.
* `npm test`: frontend display helpers (vitest).
* `npm run build`: strict TypeScript type-check plus Vite bundle.
