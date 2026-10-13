import { useState } from "react";
import { api } from "../api";
import { ScenarioPicker, useApp } from "../App";
import { fmt, Hash, Notice, OpenQuestion, Pill, Section } from "../components/common";
import type { LayerStatus, RecoveryAssessment, ScenarioView } from "../types";

function statusPill(s: LayerStatus) {
  const kind = s === "possible" ? "ok" : s === "blocked" ? "bad" : s === "partial" ? "ochre" : "neutral";
  return <Pill kind={kind}>{s}</Pill>;
}

export function Assessment({ a }: { a: RecoveryAssessment }) {
  return (
    <div>
      {a.upgrade && <div className="kicker">assessing: {a.upgrade}</div>}
      {a.layers.map((l) => (
        <div key={l.layer} className="layer">
          <div className="n">{l.layer}</div>
          <div>
            <div className="t">{l.title}</div>
            <div className="small muted">{l.question}</div>
          </div>
          <div>
            <div>
              {statusPill(l.status)} {l.finding}
            </div>
            {l.details.length > 0 && (
              <ul className="plain small">
                {l.details.map((d, i) => (
                  <li key={i}>{d}</li>
                ))}
              </ul>
            )}
          </div>
        </div>
      ))}
      <div className={`notice ${a.automatic_recovery_blocked ? "bad" : "ochre"}`}>
        <strong>{a.statement}</strong>
        <div className="small">
          automatic recovery blocked: <Pill kind={a.automatic_recovery_blocked ? "bad" : "neutral"}>{String(a.automatic_recovery_blocked)}</Pill> · code revert counts as recovery:{" "}
          <Pill kind="neutral">{String(a.code_revert_is_recovery)}</Pill> · new authorization required: <Pill kind="ochre">{String(a.requires_new_authorization)}</Pill>
        </div>
      </div>
      {a.why_code_revert_insufficient.length > 0 && (
        <div className="card quiet">
          <div className="kicker">why reverting code alone is insufficient</div>
          <ul className="plain">
            {a.why_code_revert_insufficient.map((w, i) => (
              <li key={i}>{w}</li>
            ))}
          </ul>
        </div>
      )}
      {a.actions.length > 0 && (
        <div className="card band">
          <div className="kicker">operations required, and who must authorize them</div>
          <ul className="plain">
            {a.actions.map((x, i) => (
              <li key={i}>
                {x.description} <span className="mono small">— {x.privileged ? "privileged" : "unprivileged"} · {x.authorized_by}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

export function RecoveryPage() {
  const { scenarios, active, run, setScenario, navigate, busy } = useApp();
  const v = scenarios[active];
  const [periods, setPeriods] = useState(2);
  if (!v) return <p className="muted">Loading…</p>;
  const act = (fn: () => Promise<ScenarioView>) => run(fn, setScenario);
  const executed = v.state.history.length > 0;

  return (
    <div className="page">
      <div className="row">
        <ScenarioPicker />
        <span className="kicker" style={{ margin: 0 }}>
          case {v.key} · {v.meta.package_title} · {v.status_label}
        </span>
      </div>
      <h1>Recovery, not rollback</h1>
      <p className="lead">"Rollback" usually covers layer 1. Layers 2–4 must be shown. Reverting code is never treated as a recovery; when recovery needs new privileged operations, they are proposed and authorized as a new decision.</p>
      <div className="grid-4">
        {[
          ["1 Code rollback", "Return to the prior code."],
          ["2 State compatibility", "Can the prior code read the state as it now is?"],
          ["3 External effects", "Transfers made, permissions used, integrations and users that acted."],
          ["4 Recovery", "Further transactions to reach an acceptable state; usually a new decision."],
        ].map(([t, d]) => (
          <div key={t} className="card quiet">
            <div className="kicker">{t}</div>
            <div className="small">{d}</div>
          </div>
        ))}
      </div>
      <OpenQuestion>What must a proposal establish about reversibility before it can be approved?</OpenQuestion>

      {!executed && (
        <Notice kind="ochre">
          Nothing has been executed in case {v.key} yet, so there is nothing to recover from. Run the proposal through approval and execution first.{" "}
          <button className="btn small" onClick={() => navigate({ page: "scenario", key: v.key })}>
            Stage A
          </button>
        </Notice>
      )}

      <Section title="Applied upgrades (local model)">
        {v.state.history.length === 0 ? (
          <span className="muted small">none</span>
        ) : (
          <table>
            <thead>
              <tr>
                <th>proposal</th>
                <th>change</th>
                <th>H</th>
                <th>block</th>
              </tr>
            </thead>
            <tbody>
              {v.state.history.map((h) => (
                <tr key={h.proposal_id + h.block}>
                  <td className="mono">{h.proposal_id}</td>
                  <td className="mono">
                    {h.from_version} → {h.to_version}
                  </td>
                  <td>
                    <Hash hex={h.commitment} />
                  </td>
                  <td className="mono">{fmt(h.block)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <div className="small">
          state format <span className="mono">{v.state.format_version}</span> · wallets with history <span className="mono">{fmt(v.state.wallets_with_history)}</span> · external effects recorded{" "}
          <span className="mono">{v.state.effects.length}</span>
        </div>
      </Section>

      <Section title="Let users act under the new version" kicker="external effects accumulate; nothing here is undone by reinstalling code">
        <div className="row">
          <input type="number" min={1} max={6} value={periods} onChange={(e) => setPeriods(Number(e.target.value))} style={{ width: 70 }} />
          <span className="small">periods</span>
          <button className="btn" disabled={!executed || busy || !(v.status === "executed" || v.status === "escalated")} onClick={() => act(() => api.simulateUsersAct(v.key, periods))}>
            users act
          </button>
        </div>
        {v.state.effects.length > 0 && (
          <table>
            <thead>
              <tr>
                <th>kind</th>
                <th>block</th>
                <th>effect</th>
                <th className="num">count</th>
                <th className="num">amount</th>
              </tr>
            </thead>
            <tbody>
              {v.state.effects.map((e, i) => (
                <tr key={i}>
                  <td className="mono">{e.kind.replace(/_/g, " ")}</td>
                  <td className="mono">{fmt(e.block)}</td>
                  <td>{e.description}</td>
                  <td className="num">{fmt(e.count)}</td>
                  <td className="num">{e.amount === null ? "—" : fmt(e.amount)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Section>

      <Section title="Four-layer assessment" kicker="inspectable; computed by the engine from the live model state">
        <div className="toolbar">
          <button className="btn primary" disabled={!executed || busy} onClick={() => act(() => api.assessRecovery(v.key))}>
            assess recovery
          </button>
          <button className="btn danger" disabled={!executed || busy} onClick={() => act(() => api.attemptCodeRevert(v.key))}>
            attempt "revert the code only"
          </button>
          <button className="btn ochre" disabled={!executed || busy} onClick={() => act(() => api.draftRecovery(v.key))}>
            draft a distinct recovery proposal
          </button>
        </div>
        {v.revert_attempt && (
          <div className={`notice ${v.revert_attempt.blocked ? "bad" : "ochre"}`}>
            <strong>{v.revert_attempt.blocked ? "Automatic code revert blocked." : "Code revert drafted as a new proposal."}</strong> {v.revert_attempt.reason}
            {v.revert_attempt.draft && (
              <div className="small">
                Drafted, not applied: <em>{v.revert_attempt.draft.manifest.summary}</em> It would need its own H and its own approval.
              </div>
            )}
          </div>
        )}
        {v.recovery && <Assessment a={v.recovery} />}
        {v.status === "constructed" && v.candidate?.manifest.change_kind === "recovery" && (
          <Notice kind="ochre">
            Recovery proposal {v.proposal_id} is drafted: <em>{v.candidate.manifest.summary}</em> It goes through the normal path — checks, simulation, evidence, a new H, guardian approval, execution.{" "}
            <button className="btn small" onClick={() => navigate({ page: "scenario", key: v.key })}>
              continue in Stage A
            </button>
          </Notice>
        )}
      </Section>
    </div>
  );
}
