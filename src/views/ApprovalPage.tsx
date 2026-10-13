import { useEffect, useState } from "react";
import { api } from "../api";
import { ScenarioPicker, useApp } from "../App";
import { Attachments, Hash, Kv, Notice, OpenQuestion, Pill, Section } from "../components/common";
import { EvidenceTable } from "../components/EvidenceTable";
import type { Decision, RecomputeResult, ScenarioView, SurfaceComparison, SurfaceMode, SurfaceView } from "../types";

const MODES: SurfaceMode[] = ["minimal", "structured", "full"];

function SurfaceBlock({ s, pkgItems }: { s: SurfaceView; pkgItems?: ScenarioView["package"] }) {
  return (
    <div className="card">
      <div className="kicker">{s.label}</div>
      <div className="small muted" style={{ marginBottom: 8 }}>
        {s.description}
      </div>
      <h3 style={{ marginTop: 0 }}>{s.headline}</h3>
      <div className="small">
        artifact <Hash hex={s.commitment} full /> · rows shown {s.items_shown} of {s.items_total}
      </div>
      {s.sections.map((sec) => (
        <div key={sec.title} style={{ marginTop: 10 }}>
          <div className="kicker">{sec.title}</div>
          {sec.lines.length === 0 ? (
            <div className="small muted">—</div>
          ) : (
            <ul className="plain small">
              {sec.lines.map((l, i) => (
                <li key={i}>{l}</li>
              ))}
            </ul>
          )}
        </div>
      ))}
      {s.omitted.length > 0 && (
        <div className="notice ochre" style={{ marginTop: 10 }}>
          <div className="kicker" style={{ color: "var(--ochre-2)" }}>
            what this presentation leaves out
          </div>
          <ul className="plain small">
            {s.omitted.map((o, i) => (
              <li key={i}>{o}</li>
            ))}
          </ul>
        </div>
      )}
      <div className="small muted" style={{ marginTop: 8 }}>
        {s.note}
      </div>
      {s.mode === "full" && pkgItems && (
        <details style={{ marginTop: 10 }}>
          <summary>full evidence table</summary>
          <EvidenceTable pkg={pkgItems} compact />
        </details>
      )}
      {s.attachments.length > 0 && (
        <details style={{ marginTop: 10 }}>
          <summary>artifacts, diffs, queries, test output, simulation records ({s.attachments.length})</summary>
          <Attachments items={s.attachments} />
        </details>
      )}
    </div>
  );
}

export function ApprovalPage() {
  const { ws, scenarios, active, run, setScenario, navigate, busy } = useApp();
  const v = scenarios[active];
  const [surface, setSurface] = useState<SurfaceView | null>(null);
  const [cmp, setCmp] = useState<SurfaceComparison | null>(null);
  const [compare, setCompare] = useState(false);
  const [recompute, setRecompute] = useState<RecomputeResult | null>(null);
  const [note, setNote] = useState("");
  const [lastOutcome, setLastOutcome] = useState<string | null>(null);
  const [customM, setCustomM] = useState(2);
  const [customN, setCustomN] = useState(3);

  const hasPkg = !!v?.package;
  const mode = v?.surface_mode ?? "structured";
  const key = v?.key;
  const pkgId = v?.package?.id;
  const candidateH = v?.candidate?.commitment;

  useEffect(() => {
    if (!key || !hasPkg) {
      setSurface(null);
      setCmp(null);
      return;
    }
    api.surface(key, mode).then(setSurface).catch(() => setSurface(null));
    api.compareSurfaces(key).then(setCmp).catch(() => setCmp(null));
    setRecompute(null);
  }, [key, hasPkg, mode, pkgId, candidateH]);

  useEffect(() => {
    setLastOutcome(null);
    setCompare(false);
  }, [key]);

  if (!v) return <p className="muted">Loading…</p>;
  const act = (fn: () => Promise<ScenarioView>) => run(fn, setScenario);
  const canDecide = v.status === "submitted" || v.status === "under_review";

  const decide = async (guardianId: string, decision: Decision) => {
    const out = await run(() => api.guardianDecide(v.key, guardianId, decision, note));
    if (out) {
      setLastOutcome(out.quorum_note);
      const fresh = await api.scenario(v.key);
      setScenario(fresh);
    }
  };

  return (
    <div className="page">
      <div className="row">
        <ScenarioPicker />
        <span className="kicker" style={{ margin: 0 }}>
          case {v.key} · {v.meta.package_title} · proposal {v.proposal_id} revision {v.revision} · {v.status_label}
        </span>
      </div>
      <h1>Stage B · Guardian approval</h1>
      <p className="lead">Guardians approve a change to the protocol, not a report about it. They read the package, recompute H independently of the proposer, and sign exactly H, reject it, or request changes.</p>

      {!hasPkg && (
        <Notice kind="ochre">
          No evidence package has been submitted for case {v.key} yet.{" "}
          <button className="btn small" onClick={() => navigate({ page: "scenario", key: v.key })}>
            go to Stage A
          </button>
        </Notice>
      )}

      {hasPkg && (
        <>
          <Section title="Approval surface" kicker="hypothesis, not a standard · the same proposal presented three ways; switching never changes the proposal or H">
            <div className="scale">
              <div className="end">too little</div>
              <div className="mid">
                <div className="kicker" style={{ color: "var(--ochre-2)" }}>
                  approval surface?
                </div>
                What changes in code, state, authority and value; what was not checked; what can be undone.
              </div>
              <div className="end" style={{ textAlign: "right" }}>
                too much
              </div>
            </div>
            <div className="toolbar">
              <div className="segmented">
                {MODES.map((m) => (
                  <button key={m} className={mode === m ? "active" : ""} disabled={busy} onClick={() => act(() => api.setSurfaceMode(v.key, m))}>
                    {m === "minimal" ? "A · minimal" : m === "structured" ? "B · structured" : "C · full technical evidence"}
                  </button>
                ))}
              </div>
              <button className="btn" onClick={() => setCompare(!compare)}>
                {compare ? "show the selected mode" : "compare all three side by side"}
              </button>
              {cmp && (
                <span className="mono small">
                  H identical across modes: <Pill kind={cmp.commitment_identical ? "ok" : "bad"}>{String(cmp.commitment_identical)}</Pill> package unchanged:{" "}
                  <Pill kind={cmp.package_unchanged ? "ok" : "bad"}>{String(cmp.package_unchanged)}</Pill>
                </span>
              )}
            </div>
            {compare && cmp ? (
              <div className="grid-3">
                {cmp.views.map((s) => (
                  <SurfaceBlock key={s.mode} s={s} pkgItems={v.package} />
                ))}
              </div>
            ) : (
              surface && <SurfaceBlock s={surface} pkgItems={v.package} />
            )}
            {cmp && (
              <>
                <p className="small" style={{ fontStyle: "italic" }}>
                  {cmp.working_guess}
                </p>
                <OpenQuestion>{cmp.open_question}</OpenQuestion>
                <Notice>
                  No presentation mode is asserted to be the right one. Each guardian's signature records which surface they viewed, so the reviewer can see how the surface and the decision relate.
                </Notice>
              </>
            )}
          </Section>

          <Section title="Independent recomputation of H" kicker="from the manifest and payload as held, never from the displayed value">
            <div className="toolbar">
              <button
                className="btn primary"
                disabled={busy}
                onClick={async () => {
                  const r = await run(() => api.guardianRecompute(v.key));
                  if (r) {
                    setRecompute(r);
                    const fresh = await api.scenario(v.key);
                    setScenario(fresh);
                  }
                }}
              >
                recompute H
              </button>
              <button className="btn" onClick={() => navigate({ page: "execution", key: v.key })}>
                run the independent verifier / export bundle →
              </button>
            </div>
            {recompute && (
              <div className={`notice ${recompute.matches ? "ok" : "bad"}`}>
                <Kv
                  rows={[
                    ["recomputed H", <Hash hex={recompute.computed} full />],
                    ["displayed by the agent", <Hash hex={recompute.claimed} full />],
                    ["match", <Pill kind={recompute.matches ? "ok" : "bad"}>{recompute.matches ? "identical" : "MISMATCH — the artifact held is not the one displayed"}</Pill>],
                    ["inputs", `${recompute.manifest_canonical_len} canonical manifest bytes + ${recompute.payload_len} payload bytes`],
                  ]}
                />
              </div>
            )}
          </Section>

          <Section title="Quorum policy" kicker="configurable m-of-n · the 2-of-3 preset is a demonstration choice, not a requirement established by the draft">
            <div className="row">
              <select value={v.quorum_policy.id} disabled={busy || v.stage === "execution"} onChange={(e) => act(() => api.setQuorumPolicy(v.key, e.target.value))}>
                {ws?.quorum_presets.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.label}
                  </option>
                ))}
                {!ws?.quorum_presets.some((p) => p.id === v.quorum_policy.id) && <option value={v.quorum_policy.id}>{v.quorum_policy.label}</option>}
              </select>
              <span className="mono small">custom:</span>
              <input type="number" min={1} max={9} value={customM} onChange={(e) => setCustomM(Number(e.target.value))} style={{ width: 60 }} />
              <span className="mono small">of</span>
              <input type="number" min={1} max={9} value={customN} onChange={(e) => setCustomN(Number(e.target.value))} style={{ width: 60 }} />
              <button className="btn small" disabled={busy || v.stage === "execution"} onClick={() => act(() => api.setQuorumPolicy(v.key, "", customM, customN))}>
                apply custom
              </button>
            </div>
            <div className="small">{v.quorum_policy.description}</div>
            <ul className="plain small muted">
              {v.quorum_policy.assumptions.map((a, i) => (
                <li key={i}>assumes: {a}</li>
              ))}
            </ul>
            {v.key === "B" && (
              <Notice kind="ochre">
                Case B's open question: should technical simplicity reduce approval requirements at all? The 1-of-3 preset is there to try that path; it is not endorsed.
              </Notice>
            )}
          </Section>

          <Section title="Guardian decisions" kicker="Ed25519 signatures over exactly H with local test keys · identity simulated, verification genuine">
            <div className="small muted">{v.guardians.note}</div>
            <label className="field">
              <span>note attached to the signature (optional)</span>
              <input type="text" value={note} onChange={(e) => setNote(e.target.value)} style={{ width: "100%" }} />
            </label>
            <table>
              <thead>
                <tr>
                  <th>guardian</th>
                  <th>public key</th>
                  <th>decision on record</th>
                  <th>act</th>
                </tr>
              </thead>
              <tbody>
                {v.guardians.guardians.map((g) => {
                  const a = v.approvals.find((x) => x.guardian_id === g.id);
                  return (
                    <tr key={g.id}>
                      <td>{g.label}</td>
                      <td className="mono small">{g.public_key_hex.slice(0, 16)}…</td>
                      <td>
                        {a ? (
                          <span className="small">
                            <Pill kind={a.decision === "approve" ? "ok" : a.decision === "reject" ? "bad" : "ochre"}>{a.decision.replace("_", " ")}</Pill> over <Hash hex={a.commitment} /> · viewed{" "}
                            {a.surface_mode_viewed}
                            {a.note && <span className="muted"> · “{a.note}”</span>}
                          </span>
                        ) : (
                          <span className="muted small">—</span>
                        )}
                      </td>
                      <td>
                        <button className="btn small primary" disabled={!canDecide || busy} onClick={() => decide(g.id, "approve")}>
                          approve exactly H
                        </button>
                        <button className="btn small" disabled={!canDecide || busy} onClick={() => decide(g.id, "request_changes")}>
                          request changes
                        </button>
                        <button className="btn small danger" disabled={!canDecide || busy} onClick={() => decide(g.id, "reject")}>
                          reject
                        </button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            {lastOutcome && <Notice>{lastOutcome}</Notice>}
            {v.invalidated_approvals > 0 && <div className="small muted">{v.invalidated_approvals} earlier signature(s) were invalidated by a revision or a quorum change; they do not count.</div>}
            {v.status === "changes_requested" && (
              <Notice kind="ochre">
                Changes requested. A changed proposal requires a new artifact commitment and a new approval process.{" "}
                <button className="btn small" onClick={() => navigate({ page: "scenario", key: v.key })}>
                  revise in Stage A
                </button>
              </Notice>
            )}
            {v.status === "rejected" && <Notice kind="bad">Rejected. Nothing will be executed for this H.</Notice>}
            {v.authorization && (
              <div className="card band">
                <div className="kicker">authorization (handoff: signatures over H)</div>
                <Kv
                  rows={[
                    ["proposal", <span className="mono">{v.authorization.proposal_id} revision {v.authorization.revision}</span>],
                    ["H", <Hash hex={v.authorization.commitment} full />],
                    ["policy", `${v.authorization.policy.label}`],
                    ["signatures counted", `${v.authorization.approvals.length} (${v.authorization.approvals.map((a) => a.guardian_id).join(", ")})`],
                  ]}
                />
                <button className="btn primary" onClick={() => navigate({ page: "execution", key: v.key })}>
                  hand to the executor →
                </button>
              </div>
            )}
          </Section>
        </>
      )}
    </div>
  );
}
