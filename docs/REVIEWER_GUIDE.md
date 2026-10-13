# Reviewer guide

This desktop program is a review aid for the *Guarded Upgrade Path* review
draft (October 2026). It runs offline on a deterministic local model. It is
not production software, it has not been externally reviewed, and nothing in
it answers the questions the draft asks.

## What the program is for

It makes the draft's model operable so its boundaries can be pushed:

* an autonomous agent prepares an upgrade and commits to an artifact hash `H`;
* guardians recompute `H` and sign exactly `H`, reject it, or request changes;
* a constrained executor applies only the authorized `H`;
* the six failure modes and the four upgrade cases are executable, not drawn.

## Suggested walk-through (15 minutes)

1. **Overview** — the three-stage diagram, the hypothesis stated as claims, and the assumptions with where each is tested.
2. **Case A** — press *run to submission*. Read the evidence package; click rows to see the method and what was *not* checked. Note the specification boundary.
3. **Approval surface** — switch modes A/B/C and compare side by side. `H` never changes. Press *recompute H*, then approve exactly `H` as two guardians.
4. **Execution** — execute. Then press *divert the fee* (or another substitution), execute again, and watch the refusal. Run the verifier; export a bundle and run `gup-verify` outside the app.
5. **Case B** — run it; look at the economic table and the judgment tag. Try the hypothetical 1-of-3 quorum preset and decide whether it should exist.
6. **Case C** — run, approve, execute; then *users act*, *attempt "revert the code only"* (blocked), *draft a distinct recovery proposal* and take it through the normal path.
7. **Active incident** — compare policies; change the quorum arrival block; flip the false-positive switch; try the agent buttons and the agent's attempt to lift its own containment.
8. **Where the model can fail** — run all six. Each reports expected vs observed.
9. **Questions for review** — the five questions, verbatim. Write notes; export Markdown.

## What is genuine and what is simulated

See `VERIFICATION.md` and `SIMULATION_SCOPE.md`. In one line: hash
recomputation and Ed25519 signature verification are real; guardian identity,
the protocol, the payload, the replay and every figure are simulated.

## Review scope, preserved from the draft

The external reviewer is expected to provide professional judgment based on
public systems and the presented model. The program does not ask for, and
does not need:

* confidential information from FeverTokens, GoodDollar or any other organization;
* private source code;
* production or wallet access, or any credentials;
* contract deployment or live governance participation;
* a formal security audit or certification of the architecture.

Final scope and compensation are agreed before substantive review begins.
This program does not redefine that engagement.

## Verifying outside the application

Export a verification bundle from the Execution screen, then:

```bash
gup-verify hash          --manifest manifest.json --payload payload.bin
gup-verify verify        --manifest manifest.json --payload payload.bin --expected $(cat expected_commitment.txt)
gup-verify authorization --manifest manifest.json --payload payload.bin --authorization authorization.json --guardians guardians.json
gup-verify preimage      --manifest manifest.json --payload payload.bin --out preimage.bin && sha256sum preimage.bin
```

`displayed_summary.txt` in the bundle is what the agent's interface showed.
It is not an input to any of these commands.

## Local data

Workspace and audit log live in the application data directory shown on the
*Scope & simulation* screen. Resetting a case rebuilds it from fixtures; notes
and the audit log are kept. Nothing leaves the machine.
