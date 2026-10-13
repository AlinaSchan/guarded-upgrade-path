# Requirements traceability: review draft → implementation

Source of truth: `Guarded-Upgrade-Path_review-draft.pdf` (4 pages, October
2026, prepared for Bertrand Juglas), as interpreted by the specification
correction ("Mandatory specification correction: Guarded Upgrade Path",
sections 1–16). Each row maps a requirement to the page of the draft, the
implementing files and the tests that cover it.

Paths are relative to the repository root. `D` = `crates/gup-domain/src`,
`T` = `src-tauri/src`, `F` = `src`.

## 1. Central purpose (correction §1; draft p.1)

| requirement | implementation | tests |
| --- | --- | --- |
| Interactive technical review sandbox for the preparation/authority boundary | whole program; `D/lib.rs` (`scope_statement`), `F/views/Overview.tsx` | — |
| Hypothesis stated, not presented as established; assumptions and unresolved questions visible | `D/views.rs` (`hypothesis`, `assumptions`), `F/views/Overview.tsx`, `F/views/ScopePage.tsx`, `README.md` | — |
| The demo must test where the separation fails | `D/failure_modes.rs`, `F/views/FailureModesPage.tsx` | `failure_modes::tests::every_failure_mode_behaves_as_expected` |

## 2. Three-stage authority model (correction §2; draft p.1 diagram)

| requirement | implementation | tests |
| --- | --- | --- |
| Stage A: observe, detect, diagnose, construct, checks, simulate, assemble, commit H, submit | `D/engine.rs` (`AgentStep`, `agent_step`), `D/scenarios.rs` (`observe`, `detect`, `diagnose`, `construct`, `run_checks`, `simulate`, `assemble`) | `engine::tests::agent_cannot_skip_steps_or_authorize`, `full_happy_path_a` |
| Agent holds no authority to change protocol state | no agent step mutates `state` except through `execute`; `D/engine.rs` | `engine::tests::agent_cannot_skip_steps_or_authorize` |
| Stage B: inspect, recompute H, inspect checked/assumptions, consequences, approve exactly H, reject, request changes | `D/engine.rs` (`guardian_inspect`, `guardian_recompute`, `guardian_decide`), `D/guardians.rs`, `F/views/ApprovalPage.tsx` | `guardians::tests::*`, `engine::tests::guardian_recompute_detects_pre_signing_substitution` |
| Changed proposal → new commitment, new approval | `D/engine.rs` (`AgentStep::Revise`, revision bump, approvals invalidated) | `engine::tests::changes_requested_invalidates_approvals_and_changes_h` |
| Configurable m-of-n; 2-of-3 as a preset only | `D/guardians.rs` (`quorum_presets`, `QuorumPolicy::custom`), `D/engine.rs` (`set_quorum_policy`) | `engine::tests::quorum_policy_is_configurable` |
| Proposer never sets its own approval requirements or risk class | no such field exists; `docs/POLICIES.md` | — |
| Stage C: authorization referring to H, independent verification, apply only the payload, report, post-state verification, escalate | `D/executor.rs` | `engine::tests::full_happy_path_a`, `substitution_after_approval_is_refused` |
| Executor must not modify or substitute the payload; frontend never decides authorization | `D/executor.rs` (installs `artifact.bundle()` as handed), `T/desktop.rs` (commands only forward) | `engine::tests::substitution_after_approval_is_refused` |
| Primary visualization follows the p.1 diagram | `F/views/Overview.tsx`, `F/styles.css` (`.diagram`) | — |

## 3. Artifact commitment and independent verification (correction §3; draft p.2 "approved payload hash", p.4 "Reviewed is not executed")

| requirement | implementation | tests |
| --- | --- | --- |
| Deterministic, versioned manifest + executable artifact as deterministic example data | `D/artifact.rs` (`gup-manifest/1`, `gup-implbundle/1`), `D/fixtures.rs` | `artifact::tests::*`, `fixtures::tests::*` |
| Canonical serialization format | `D/canonical.rs`, `docs/VERIFICATION.md` | `canonical::tests::*` |
| H computed from the exact manifest and payload; reproducible | `D/canonical.rs` (`compute_commitment`, `commitment_preimage`) | `canonical::tests::commitment_*`, `artifact::tests::payload_roundtrip_and_commitment_reproducible` |
| Independent verification path that does not trust the displayed summary or claimed hash | `D/verifier.rs`, `D/engine.rs` (`verify_independently`, `verification_bundle`) | `crates/gup-verify/tests/cli.rs` |
| Separately invokable verifier CLI | `crates/gup-verify/src/main.rs` (`hash`, `verify`, `authorization`, `preimage`, `inspect`) | `crates/gup-verify/tests/cli.rs` |
| Reads actual inputs, recomputes, reports, compares, verifies payload, detects post-approval modification, rejects mismatches | `D/verifier.rs`, `D/checks.rs` (`manifest_binding_check`) | `cli.rs`, `engine::tests::substitution_after_approval_is_refused` |
| Demonstrable attack: UI shows one upgrade, artifact changed → execution rejected | `D/engine.rs` (`tamper`, `DisplayedSummary` kept apart), `D/scenarios.rs` (`tamper`), `F/views/ExecutionPage.tsx` | `failure_modes` F5, `engine::tests::substitution_after_approval_is_refused` |
| Real signature verification with clearly identified local test keys; simulated selection not presented as authentication | `D/guardians.rs` (`GuardianSet::local_test`, labels, `note`), `docs/VERIFICATION.md` | `guardians::tests::signatures_verify_and_bind_commitment` |
| Document what is cryptographic and what is simulated | `docs/VERIFICATION.md`, `F/views/ScopePage.tsx` | — |

## 4. Evidence package from page 2 (correction §4)

| requirement | implementation | tests |
| --- | --- | --- |
| Main scenario Distribution 2.3.0 → 2.3.1, duplicate payouts at rollover | `D/scenarios.rs` (`ScenarioId::CaseA`, `assemble`), `D/fixtures.rs` | `simulation::tests::buggy_implementation_pays_fourteen_wallets_twice` |
| 14 wallets over 3 days; one body changes; no selector changes; storage unchanged; `lastPaidPeriod` meaning asserted; no new roles; checks incl. uniqueness; 30-day replay 41,200 claims no duplicates; ≈0.3 % daily overpayment avoided; past overpayments not recovered; scheduler assumptions unresolved; reverting restores bug | `D/simulation.rs` (`distribution_fixture_stream`, `trigger_wallets`), `D/scenarios.rs` (case A rows) | `simulation::tests::*` (14 duplicates, periods 27–29, 20–40 bps, 0 duplicates after fix, differs only for affected wallets) |
| Label values as illustrative fixtures; describe the replay model accurately; show scope of executed checks | `EvidencePackage::fixture_note`, `simulation_scope`; `CheckResult::scope`; `docs/SIMULATION_SCOPE.md` | — |

## 5. Four evidence classifications (correction §5; draft p.2 tags)

| requirement | implementation | tests |
| --- | --- | --- |
| CHECKED / SIMULATED / ASSUMED / JUDGMENT with definitions; multiple per row; never collapsed | `D/evidence.rs` (`Classification`, `EvidenceItem.classifications`), `F/components/EvidenceTable.tsx` (tags, legend) | `failure_modes` F2, F3 (classification assertions) |
| Each item: claim, classification, evidence, method, not checked, establishes, remaining uncertainty | `D/evidence.rs` (`EvidenceItem`), `D/scenarios.rs` (`item`) | — |
| Technically valid vs desirable distinguished | `EvidencePackage::technical_verdict` / `desirability`, `Concern`, `F/components/EvidenceTable.tsx` | `failure_modes` F3 |

## 6. Approval surface experiment (correction §6; draft p.2 "Approval surface")

| requirement | implementation | tests |
| --- | --- | --- |
| Mode A minimal, explains what is omitted | `D/surface.rs` (`project`, `omitted`) | `engine::tests::surfaces_never_change_h`, `failure_modes` F2 |
| Mode B structured with the ten sections | `D/surface.rs` (sections: code, state, authority, economic, verification, assumptions, simulation scope, reversibility, recovery limitations, exact commitment) | F2 |
| Mode C full technical evidence | `D/surface.rs` (attachments: diffs, queries, test output, simulation records, manifest, payload, checks) | — |
| Switching never changes the proposal or H | `D/surface.rs` (pure projection), `D/engine.rs` (`set_surface_mode` debug assertion) | `engine::tests::surfaces_never_change_h` |
| Reviewer-facing comparison; no mode asserted correct | `D/surface.rs` (`compare`), `F/views/ApprovalPage.tsx` | — |

## 7. Four upgrade cases (correction §7; draft p.3)

| requirement | implementation | tests |
| --- | --- | --- |
| Case A implementation change, state unchanged; passing checks ≠ desirable | `D/scenarios.rs` (CaseA), `D/fixtures.rs` (2.3.0 / 2.3.1) | `simulation::tests::*`, `engine::tests::full_happy_path_a` |
| Case B parameter-only; material user effect; revert ≠ reversal; economic comparison | `D/scenarios.rs` (CaseB, `EconomicComparison`), `D/fixtures.rs` (`distribution_bundle_2_3_1_fee`) | `failure_modes` F3 |
| Case C migration; code rollback / state compatibility / external effects / recovery tracked separately; recovery ≠ revert | `D/scenarios.rs` (CaseC), `D/protocol.rs` (`migrate_to_format2`, misreads), `D/recovery.rs` | `engine::tests::migration_then_revert_is_blocked`, F4 |
| Case D incident; quorum not reachable; consequences of waiting and premature action; authority ladder; open boundary; selectable hypothetical policies; normal default; no silent emergency privileges; pause side effects | `D/incident.rs`, `D/scenarios.rs` (CaseD, `exploit_stream`), `D/engine.rs` (incident actions), `F/views/IncidentPage.tsx` | `incident::tests::*`, `engine::tests::incident_authority_is_bounded`, F6 |

## 8. Recovery model (correction §8; draft p.3 layers 1–4)

| requirement | implementation | tests |
| --- | --- | --- |
| Four inspectable layers | `D/recovery.rs` (`assess_upgrade`) | `engine::tests::migration_then_revert_is_blocked`, F4 |
| New privileged actions → distinct proposal and authorization | `D/recovery.rs` (`draft_recovery_proposal`), `D/engine.rs` (`draft_recovery` → new proposal id) | `engine::tests::migration_then_revert_is_blocked` |
| Unsafe automatic recovery blocked; UI shows why code revert is insufficient | `D/recovery.rs` (`attempt_code_revert` → `Blocked`), `F/views/RecoveryPage.tsx` | F4 |

## 9. Six failure modes (correction §9; draft p.4)

| failure mode | implementation | test |
| --- | --- | --- |
| F1 proof holds, property wrong | `D/failure_modes.rs`, `D/fixtures.rs` (`distribution_bundle_2_3_1_flat`), `CandidateVariant::FlatAmount` | `every_failure_mode_behaves_as_expected` |
| F2 checks pass, judgment stops | `D/failure_modes.rs`, `D/surface.rs` | same |
| F3 valid code, unwanted economics | `D/failure_modes.rs`, case B | same |
| F4 rollback that isn't | `D/failure_modes.rs`, case C, `D/recovery.rs` | same |
| F5 reviewed is not executed | `D/failure_modes.rs`, `D/engine.rs` (`tamper`), `D/executor.rs` | same + `cli.rs` |
| F6 quorum late / authority creeps | `D/failure_modes.rs`, `D/incident.rs` | same |
| Executable through the engine, not frontend animation | `failure_modes::run` builds a fresh `Workspace` | same |

## 10. Review Lab (correction §10)

| requirement | implementation |
| --- | --- |
| Select case; compare surfaces; verify H; checked vs assumed; modified payloads; quorum policies; economic effect; incident limits; failed execution; rollback vs recovery; deterministic reset; export observations | `F/views/ReviewLab.tsx` (links to each tool), `D/engine.rs` (`reset_scenario`, `reset_all`), `D/exports.rs` |
| Local review notes with the five fields, Markdown export, never presented as supplied/approved | `D/notes.rs`, `T/desktop.rs` (`add_note`, `export_notes_markdown`), `F/views/NotesPage.tsx` |

## 11. Questions for review (correction §11; draft p.4)

| requirement | implementation | tests |
| --- | --- | --- |
| Exact five questions, each linked to scenarios, unresolved | `D/questions.rs`, `F/views/QuestionsPage.tsx` | `questions::tests::five_questions_each_linked_and_unresolved` |

## 12. Visual language (correction §12)

Light paper background, graphite text, serif typography, monospace tags,
ochre only for authority handoffs and open questions, structured evidence
tables, dark appearance optional: `F/styles.css`, `F/views/*.tsx`. No crypto
dashboards, charts or wallet widgets.

## 13. Review scope preserved (correction §13; draft p.4 "Proposed review scope")

`F/views/ScopePage.tsx`, `F/views/QuestionsPage.tsx`, `docs/REVIEWER_GUIDE.md`, `README.md`: not required list reproduced; the program is a review aid.

## 14. Over-specified policies corrected (correction §14)

`docs/POLICIES.md` (table of replacements), `D/guardians.rs`, `D/incident.rs`, `D/recovery.rs`.

## 15. Acceptance criteria (correction §15)

| # | criterion | where | status |
| --- | --- | --- | --- |
| 1 | agent prepares but cannot authorize | `D/engine.rs` | tested |
| 2 | reproducible H | `D/canonical.rs`, CLI `preimage` | tested |
| 3 | approval bound to the exact artifact | `D/guardians.rs` | tested |
| 4 | independent verifier detects substitution | `crates/gup-verify` | tested (out of process) |
| 5 | executor cannot apply a payload ≠ H | `D/executor.rs` | tested |
| 6 | four classifications | `D/evidence.rs` | tested |
| 7 | Distribution 2.3.0 → 2.3.1 example | `D/scenarios.rs` | tested |
| 8 | four cases explorable | `D/scenarios.rs`, frontend | tested (engine) |
| 9 | six failure modes with automated coverage | `D/failure_modes.rs` | tested |
| 10 | economics separate from technical correctness | `D/evidence.rs`, F3 | tested |
| 11 | recovery covers code, state, effects, authorization | `D/recovery.rs` | tested |
| 12 | incident powers scoped, documented as hypotheses | `D/incident.rs`, `docs/POLICIES.md` | tested |
| 13 | five questions link to functionality | `D/questions.rs` | tested |
| 14 | local persistence of state and audit | `T/desktop.rs` | verified by running the desktop app: `workspace.json` and `audit.jsonl` written to the app data directory |
| 15 | notes export | `T/desktop.rs`, `D/notes.rs` | Markdown generation covered by the engine; the native save dialog path was not exercised automatically |
| 16 | offline after installation | no network code anywhere | by construction |
| 17 | Windows and macOS release configurations | `src-tauri/tauri.*.conf.json`, `.github/workflows/release.yml` | configured; Windows NSIS and MSI bundles built locally (GNU toolchain); macOS bundles only via the workflow (no macOS machine available) |
| 18 | Linux installers not published; the application does not run on Linux | `tauri.linux.conf.json` (`bundle.active=false`), no Linux job; `T/main.rs` + `T/unsupported.rs` (on any target other than Windows/macOS the binary only prints an error and exits; Tauri is not compiled there) | configured; the refusal is checked in CI on Linux |
| 19 | simulation scope documented | `docs/SIMULATION_SCOPE.md` | — |
| 20 | README makes no production/review claims | `README.md` | — |

## Missing information and implementation assumptions

* **The earlier implementation prompt was not available** to this work. Its stated areas (Tauri desktop framework, supported operating systems, GitHub packaging, persistence, testing, code quality) were implemented from the correction's description: Tauri 2, Windows + macOS release configurations, JSON persistence in the app data directory, Rust and frontend tests, GitHub Actions. No previous code base existed in the repository folder; the application was implemented from scratch.
* **No real bytecode.** The draft's "bytecode" is represented by a canonical-JSON implementation bundle interpreted through a registry of implementation ids. H is computed over those bytes exactly as it would be over bytecode.
* **Selectors** are derived from SHA-256 rather than Keccak-256; the model only needs a stable mapping.
* **The replay is a local model**, not a fork; "41,200 claims" are synthetic. The affected-wallet count, period range and ≈0.3 % figure are engineered fixtures that reproduce the draft's hypothetical example.
* **The draft's illustrative hash `0x7c1e…a94d`** is not reproduced; the program shows the real computed H and labels the draft's value illustrative.
* **Guardian identity is simulated** with local test keys; the draft does not specify a key scheme.
* **Timelocks are not modelled**; quorum arrival time is a parameter of case D.
* **Economic figures** (fee 1.00 % → 2.50 %, tiers, unit amounts) are inventions of the model; the draft gives no numbers for case B.
* **Case C's feature request (FR-12) and case B's governance request (GR-7)** are invented triggers; the draft describes the cases abstractly.
* **The incident timeline** is a closed-form model (detection, containment, quorum, lapse), not a block-by-block simulation of an attacker.
* **Signing/notarization** of installers is not configured; releases are drafts and unsigned.
