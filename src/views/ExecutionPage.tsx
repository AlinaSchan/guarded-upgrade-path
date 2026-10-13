import { useEffect, useState } from "react";
import { chooseDirectory, revealPath } from "../platform";
import { api } from "../api";
import { ScenarioPicker, useApp } from "../App";
import { fmt, Hash, Kv, Notice, Pill, Section } from "../components/common";
import { ChecksTable } from "../components/EvidenceTable";
import type { ExportResult, ScenarioView, TamperVariant, VerifierReport } from "../types";

const TAMPERS: { v: TamperVariant; label: string; desc: string }[] = [
  { v: "divert_fee", label: "divert the fee", desc: "payload body changed; manifest and displayed summary unchanged" },
  { v: "reroute_selector", label: "reroute a selector", desc: "claim selector routed to the privileged rollover body" },
  { v: "silent_parameter", label: "silent parameter", desc: "feeBps 900 inside the payload; the summary still shows the reviewed value" },
  { v: "manifest_and_payload", label: "manifest + payload, consistent", desc: "internal binding passes; only H exposes it" },
];

export function VerifierReportView({ r }: { r: VerifierReport }) {
  return (
    <div className={`notice ${r.verdict === "accept" ? "ok" : "bad"}`}>
      <div>
        verdict <Pill kind={r.verdict === "accept" ? "ok" : "bad"}>{r.verdict}</Pill>
        {r.reasons.length > 0 && <span className="small"> — {r.reasons.join("; ")}</span>}
      </div>
      <ul className="plain small">
        {r.steps.map((s, i) => (
          <li key={i}>
            <Pill kind={s.ok ? "ok" : "bad"}>{s.ok ? "ok" : "FAIL"}</Pill> {s.name}: <span className="mono">{s.detail}</span>
          </li>
        ))}
        {r.signatures.map((s, i) => (
          <li key={`s${i}`}>
            <Pill kind={s.ok ? "ok" : "bad"}>{s.ok ? "ok" : "FAIL"}</Pill> signature {s.guardian_id} ({s.decision}): <span className="mono">{s.detail}</span>
          </li>
        ))}
      </ul>
      {r.quorum_need > 0 && (
        <div className="small mono">
          quorum: {r.quorum_have} of {r.quorum_need} valid approvals over the recomputed H
        </div>
      )}
      <div className="small">
        computed <Hash hex={r.computed_commitment} full />
      </div>
      {r.expected_commitment && (
        <div className="small">
          expected <Hash hex={r.expected_commitment} full />
        </div>
      )}
      <div className="small muted" style={{ marginTop: 6 }}>
        establishes: {r.what_this_establishes}
      </div>
      <div className="small muted">does not establish: {r.what_this_does_not_establish}</div>
    </div>
  );
}

export function ExecutionPage() {
  const { scenarios, active, run, setScenario, navigate, busy } = useApp();
  const v = scenarios[active];
  const [verifier, setVerifier] = useState<VerifierReport | null>(null);
  const [exported, setExported] = useState<ExportResult | null>(null);
  const artifactKey = `${v?.key ?? ""}:${v?.candidate?.commitment ?? ""}:${v?.proposal_id ?? ""}`;
  useEffect(() => {
    setVerifier(null);
    setExported(null);
  }, [artifactKey]);
  if (!v) return <p className="muted">Loading…</p>;
  const act = (fn: () => Promise<ScenarioView>) => run(fn, setScenario);
  const canTamper = ["committed", "submitted", "under_review", "approved", "execution_refused"].includes(v.status);
  const canExecute = (v.status === "approved" || v.status === "execution_refused") && !!v.authorization;
  const claimed = v.displayed_summary?.claimed_commitment;
  const held = v.candidate?.commitment;
  const mismatch = claimed && held && claimed !== held;

  const exportBundle = async () => {
    const dir = await chooseDirectory();
    if (!dir) return;
    const r = await run(() => api.exportVerificationBundle(v.key, dir));
    if (r) setExported(r);
  };

  return (
    <div className="page">
      <div className="row">
        <ScenarioPicker />
        <span className="kicker" style={{ margin: 0 }}>
          case {v.key} · {v.meta.package_title} · proposal {v.proposal_id} · {v.status_label}
        </span>
      </div>
      <h1>Stage C · Execution and independent verification</h1>
      <p className="lead">The executor receives an authorization that refers to H, independently verifies that the artifact it is handed hashes to H, applies only that payload, reports the outcome, and escalates an unexpected post-state. It cannot modify or substitute the payload, and the frontend never decides whether execution is authorized.</p>

      <Section title="What the executor holds" kicker="artifact as held vs. what the agent's interface displays">
        <div className="grid-2">
          <div className="card">
            <div className="kicker">artifact as held (manifest + payload)</div>
            {v.candidate ? (
              <Kv
                rows={[
                  ["package", <span className="mono">{v.candidate.manifest.package} {v.candidate.manifest.from_version} → {v.candidate.manifest.to_version}</span>],
                  ["H recomputed now", <Hash hex={v.candidate.commitment} full />],
                  ["payload", `${fmt(v.candidate.payload_len)} bytes`],
                ]}
              />
            ) : (
              <span className="muted">no candidate yet</span>
            )}
          </div>
          <div className="card">
            <div className="kicker">displayed by the agent's interface (never an input to verification)</div>
            {v.displayed_summary ? (
              <Kv
                rows={[
                  ["title", v.displayed_summary.title],
                  ["displayed H", <Hash hex={v.displayed_summary.claimed_commitment} full />],
                  ["rendered by", v.displayed_summary.rendered_by],
                ]}
              />
            ) : (
              <span className="muted">nothing rendered yet</span>
            )}
          </div>
        </div>
        {mismatch && <Notice kind="bad">The artifact held does not hash to the displayed value. The interface shows one upgrade; the payload is another.</Notice>}
        {v.authorization ? (
          <div className="card band">
            <div className="kicker">authorization received</div>
            <Kv
              rows={[
                ["refers to H", <Hash hex={v.authorization.commitment} full />],
                ["policy", v.authorization.policy.label],
                ["signatures", v.authorization.approvals.map((a) => a.guardian_id).join(", ")],
              ]}
            />
          </div>
        ) : (
          <Notice kind="ochre">
            No authorization exists for case {v.key}. The executor has nothing it may apply.{" "}
            <button className="btn small" onClick={() => navigate({ page: "approval", key: v.key })}>
              go to Stage B
            </button>
          </Notice>
        )}
      </Section>

      <Section title="Attack: substitute the artifact after review" kicker="reviewed is not executed · the displayed summary stays as it was">
        <p>Guardians approve one artifact and another one runs. If the proposer also renders the summary, a hash only relocates the trust. The correct outcome is execution rejection.</p>
        <div className="row">
          {TAMPERS.map((t) => (
            <button key={t.v} className="btn danger" disabled={!canTamper || busy} title={t.desc} onClick={() => act(() => api.tamper(v.key, t.v))}>
              {t.label}
            </button>
          ))}
          {v.tamper && (
            <button className="btn" disabled={busy} onClick={() => act(() => api.restoreCandidate(v.key))}>
              restore the original artifact
            </button>
          )}
        </div>
        {v.tamper && (
          <Notice kind="bad">
            <strong>{v.tamper.variant.replace(/_/g, " ")}</strong>: {v.tamper.description}
            <div className="small">
              approved/displayed <Hash hex={v.tamper.original_commitment} /> · now held <Hash hex={v.tamper.tampered_commitment} />
            </div>
          </Notice>
        )}
      </Section>

      <Section title="Execute" kicker="the executor recomputes H, verifies signatures and quorum, then applies the payload it was handed">
        <div className="toolbar">
          <button className="btn primary" disabled={!canExecute || busy} onClick={() => act(() => api.execute(v.key))}>
            executor: apply authorized artifact
          </button>
          {!canExecute && <span className="small muted">requires status approved (or a refused execution to retry)</span>}
        </div>
        {v.execution?.outcome === "refused" && (
          <div className="notice bad">
            <strong>Execution refused. Nothing was applied.</strong>
            {v.execution.refusal.kind === "commitment_mismatch" && (
              <div className="small">
                expected <Hash hex={v.execution.refusal.expected} full /> · computed <Hash hex={v.execution.refusal.computed} full />
              </div>
            )}
            {v.execution.refusal.kind === "wrong_proposal" && (
              <div className="small">
                authorization refers to {v.execution.refusal.expected}; execution requested for {v.execution.refusal.got}
              </div>
            )}
            {v.execution.refusal.kind === "payload_rejected" && <div className="small">{v.execution.refusal.reason}</div>}
            {(v.execution.refusal.kind === "commitment_mismatch" || v.execution.refusal.kind === "authorization_invalid") && <VerifierReportView r={v.execution.refusal.verifier} />}
          </div>
        )}
        {v.execution?.outcome === "applied" && (
          <div className={`notice ${v.execution.report.post_state.ok ? "ok" : "bad"}`}>
            <strong>{v.execution.report.statement}</strong>
            <Kv
              rows={[
                ["installed", <span className="mono">{v.execution.report.install.from_version} → {v.execution.report.install.to_version} · state format {v.execution.report.install.format_version} · migrated entries {fmt(v.execution.report.install.migrated_entries)}</span>],
                ["block", fmt(v.execution.report.block)],
                ["smoke replay", v.execution.report.post_state.smoke_replay],
              ]}
            />
            {v.execution.report.post_state.escalation && <Notice kind="bad">{v.execution.report.post_state.escalation}</Notice>}
            <details>
              <summary>post-state verification checks ({v.execution.report.post_state.checks.length})</summary>
              <ChecksTable checks={v.execution.report.post_state.checks} />
            </details>
            <details>
              <summary>verifier report used by the executor</summary>
              <VerifierReportView r={v.execution.report.verifier} />
            </details>
            <div className="row">
              <button className="btn" onClick={() => navigate({ page: "recovery", key: v.key })}>
                what if this has to be undone? → recovery
              </button>
            </div>
          </div>
        )}
        {v.status === "escalated" && <Notice kind="ochre">Unexpected post-state returned to the guardians: recovery is a new decision, not a rollback.</Notice>}
      </Section>

      <Section title="Independent verifier" kicker="reads the tagged artifact inputs only · never the displayed summary or the claimed hash">
        <div className="toolbar">
          <button
            className="btn"
            disabled={!v.candidate || busy}
            onClick={async () => {
              const r = await run(() => api.verifyIndependently(v.key));
              if (r) setVerifier(r);
            }}
          >
            run verifier in-process
          </button>
          <button className="btn" disabled={!v.candidate || busy} onClick={exportBundle}>
            export verification bundle for gup-verify (CLI)
          </button>
        </div>
        {verifier && <VerifierReportView r={verifier} />}
        {exported && (
          <div className="card quiet">
            <div className="kicker">bundle written</div>
            <div className="mono small">{exported.directory}</div>
            <ul className="plain small mono">
              {exported.files.map((f) => (
                <li key={f}>{f}</li>
              ))}
            </ul>
            <button className="btn small" onClick={() => revealPath(exported.files[0] ?? exported.directory).catch(() => undefined)}>
              reveal in file manager
            </button>
          </div>
        )}
        <div className="card quiet">
          <div className="kicker">out of process, with the separately built gup-verify binary</div>
          <pre>{`gup-verify hash          --manifest manifest.json --payload payload.bin
gup-verify verify        --manifest manifest.json --payload payload.bin --expected <H from the authorization>
gup-verify authorization --manifest manifest.json --payload payload.bin --authorization authorization.json --guardians guardians.json
gup-verify preimage      --manifest manifest.json --payload payload.bin --out preimage.bin   # then: sha256sum preimage.bin`}</pre>
          <div className="small muted">
            Exit code 0 = verified, 2 = rejected. The in-process run above uses the same verification code on the artifact as held; the CLI is the path that does not share a process with this
            application. See docs/VERIFICATION.md for what is cryptographically verified and what remains simulated.
          </div>
        </div>
      </Section>
    </div>
  );
}
