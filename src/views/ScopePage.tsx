import { useEffect, useState } from "react";
import { api } from "../api";
import { useApp } from "../App";
import { Notice, Section } from "../components/common";

export function ScopePage() {
  const { ws } = useApp();
  const [dir, setDir] = useState<string>("");
  useEffect(() => {
    api.dataDir().then(setDir).catch(() => setDir(""));
  }, []);
  if (!ws) return null;
  return (
    <div className="page">
      <h1>Scope and simulation</h1>
      <p className="lead">{ws.scope_statement}</p>

      <Section title="What is genuinely verified">
        <ul className="plain">
          <li>
            <strong>Artifact commitment H.</strong> SHA-256 over a canonical serialization of the manifest and the raw payload bytes, with length framing and a domain prefix. Reproducible by anyone from the
            files; the CLI can write the exact preimage for <span className="mono">sha256sum</span>.
          </li>
          <li>
            <strong>Guardian signatures.</strong> Ed25519 signatures over a domain-separated message binding proposal id, revision, decision and H. Verified with the guardians' public keys by the engine, the
            executor and the CLI.
          </li>
          <li>
            <strong>Manifest–payload binding.</strong> The manifest's declared functions, storage layout, parameters, roles, migration and payload hash are compared with the parsed payload.
          </li>
          <li>
            <strong>Structural checks.</strong> Selector uniqueness, interface conformance, grow-only typed storage (or migration consistency), writes to own slots, roles and privilege flags — all on the
            parsed payload.
          </li>
          <li>
            <strong>State machine and authority boundaries.</strong> Enforced in the Rust engine: the agent cannot authorize; the executor applies only an authorized H; emergency grants are policy-bound,
            scoped, and lifted only by guardians.
          </li>
        </ul>
      </Section>

      <Section title="What is simulated">
        <ul className="plain">
          <li>
            <strong>The protocol.</strong> A small in-memory model of one package (Distribution) in a diamond-style system. No chain, no fork, no EVM, no gas.
          </li>
          <li>
            <strong>The payload.</strong> Deterministic example data (a canonical JSON implementation bundle with pseudo-source bodies) standing in for bytecode; behaviour is interpreted by a registry of
            known implementation ids.
          </li>
          <li>
            <strong>The replay.</strong> A seeded synthetic claim stream (41,200 claims over 30 periods for case A). "Results differ only for the 14 affected wallets" is a statement about this stream.
          </li>
          <li>
            <strong>Fuzzing.</strong> Random seeds, keeper lag and repeat claims on the local model; it establishes only what held for the cases run.
          </li>
          <li>
            <strong>Guardian identity.</strong> Local test keys derived from public seeds; the demo holds every private key. Selecting a guardian in the UI is not authentication.
          </li>
          <li>
            <strong>Economic figures, incident losses, timelines.</strong> Illustrative parameters of the model.
          </li>
        </ul>
      </Section>

      <Section title="What the review draft does not establish, and this program does not assume">
        <ul className="plain">
          <li>A fixed 2-of-3 quorum (a demonstration preset here; configurable m-of-n).</li>
          <li>A mandatory timelock (none is modelled; quorum arrival time is a parameter in case D).</li>
          <li>Fixed incident-pause authority (hypothetical policies only; normal approval is the default).</li>
          <li>Automatically permitted rollback (replaced by the four-layer recovery model; automatic recovery is blocked when layers 2–4 say so).</li>
          <li>A single global definition of low-risk, or agent-controlled risk classification (the proposer never sets its own scrutiny level).</li>
        </ul>
      </Section>

      <Section title="Review scope (preserved from the draft)">
        <Notice>
          The external reviewer is expected to provide professional judgment based on public systems and the presented model. <strong>Not required:</strong> confidential information from FeverTokens, GoodDollar or
          any other organization; source code review of private systems; production or wallet access; credentials; deployment; live governance participation; a formal security audit; certification of the
          architecture. This desktop program is a review aid and does not redefine the agreed consulting engagement.
        </Notice>
      </Section>

      <Section title="Local data">
        <div className="small">
          workspace and audit log: <span className="mono">{dir || "…"}</span>
        </div>
        <div className="small muted">
          The desktop application runs offline: nothing is sent anywhere, and exports go only where the reviewer chooses through the native dialogs. It is built for Windows
          and macOS only; there is no browser or Linux version. Version {ws.app_version}.
        </div>
      </Section>
    </div>
  );
}
