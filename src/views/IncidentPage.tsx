import { useState } from "react";
import { api } from "../api";
import { useApp } from "../App";
import { fmt, Notice, OpenQuestion, Pill, Section } from "../components/common";
import type { IncidentOutcome, IncidentParams, Rung, ScenarioView } from "../types";

const RUNG_ORDER: Rung[] = ["flag", "draft_fix", "limit_one_function", "pause_one_package", "pause_protocol"];

function rungIndex(r: Rung | null): number {
  return r ? RUNG_ORDER.indexOf(r) : -1;
}

export function IncidentPage() {
  const { ws, scenarios, run, setScenario, navigate, busy, setActive } = useApp();
  const v = scenarios["D"];
  const [params, setParams] = useState<IncidentParams | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  if (!v || !ws) return <p className="muted">Loading…</p>;
  const inc = v.incident;
  if (!inc) return <Notice>Case D has no incident state.</Notice>;
  const p = params ?? inc.params;
  const policy = ws.incident_policies.find((x) => x.id === inc.policy_id) ?? ws.incident_policies[0];
  // Rungs 1-2 (flag, draft) change no protocol state and are ordinary agent work under every policy.
  const allowed = Math.max(1, rungIndex(policy.pre_authorized));
  const shown: IncidentOutcome | undefined = inc.outcomes.find((o) => o.policy.id === (selected ?? inc.policy_id)) ?? inc.outcomes[0];
  const act = (fn: () => Promise<ScenarioView>) => run(fn, setScenario);
  const setP = (patch: Partial<IncidentParams>) => setParams({ ...p, ...patch });

  const request = async (r: Rung) => {
    const res = await run(() => api.incidentAgentRequest("D", r));
    if (res) {
      setMessage(res.message);
      setScenario(res.scenario);
    }
  };

  return (
    <div className="page">
      <div className="kicker">case D · {v.meta.package_title} · incident {inc.incident_id} ({inc.open ? "open" : "closed"})</div>
      <h1>Active incident</h1>
      <p className="lead">A fault is being exploited now. A candidate patch may exist before enough signers can be reached. Waiting has a cost, and so does acting without quorum. A pause has side effects of its own: users cannot exit, integrations stop, distributions halt.</p>
      <Notice kind="ochre">
        Normal approval remains the default. Every other policy here is an explicitly hypothetical configuration for comparison. The agent never silently receives emergency privileges; any grant is scoped to this incident, bounded in blocks, and lifted only by guardians.
      </Notice>

      <Section title="Authority ladder" kicker="bounded authority, if any · candidates, narrow to broad">
        <table>
          <thead>
            <tr>
              <th>mode</th>
              <th>who does what</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>Normal</td>
              <td>Agent observes, analyzes and prepares a proposal.</td>
            </tr>
            <tr>
              <td>Guardian review</td>
              <td>Agent supplies evidence, answers deterministic queries and re-runs checks.</td>
            </tr>
            <tr className="decision">
              <td>Incident</td>
              <td>
                Bounded authority, if any. Candidates, narrow to broad:
                <div className="ladder" style={{ marginTop: 6 }}>
                  {ws.rungs.map((r, i) => (
                    <span key={r.rung}>
                      <span className={`rung ${i <= allowed ? "allowed" : ""}`} title={r.side_effects}>
                        {r.number} {r.label}
                      </span>
                      {i < ws.rungs.length - 1 && <span className="sep"> › </span>}
                    </span>
                  ))}
                </div>
                <div className="small muted" style={{ marginTop: 4 }}>
                  highlighted: ordinary agent work (flag, draft a fix: no protocol state change) plus what the selected policy pre-authorizes (
                  {policy.pre_authorized && rungIndex(policy.pre_authorized) > 1 ? policy.pre_authorized.replace(/_/g, " ") : "nothing that changes protocol state"})
                </div>
              </td>
            </tr>
            <tr>
              <td>Quorum</td>
              <td>Guardians authorize an upgrade, a pause, a recovery path, or no action.</td>
            </tr>
          </tbody>
        </table>
        <OpenQuestion>Where on the incident rung does pre-authorization stop, and who lifts a pause once it is in place?</OpenQuestion>
      </Section>

      <Section title="Policy under comparison" kicker="selectable, explicitly hypothetical configurations">
        <div className="row">
          <select value={inc.policy_id} disabled={busy} onChange={(e) => act(() => api.incidentSetPolicy("D", e.target.value))}>
            {ws.incident_policies.map((pol) => (
              <option key={pol.id} value={pol.id}>
                {pol.label}
                {pol.is_default ? "" : " — hypothetical"}
              </option>
            ))}
          </select>
          <span className="small muted">max duration {policy.max_duration_blocks || "n/a"} blocks · lift requires guardians: {String(policy.lift_requires_guardians)}</span>
        </div>
        <ul className="plain small">
          {policy.assumptions.map((a, i) => (
            <li key={i}>assumes: {a}</li>
          ))}
        </ul>
      </Section>

      <Section title="Scenario parameters" kicker="illustrative numbers of the local model">
        <div className="grid-4">
          <label className="field">
            <span>exploit loss per block (units)</span>
            <input type="number" value={p.exploit_loss_per_block} onChange={(e) => setP({ exploit_loss_per_block: Number(e.target.value) })} />
          </label>
          <label className="field">
            <span>quorum reachable at block</span>
            <input type="number" value={p.quorum_arrival_block} onChange={(e) => setP({ quorum_arrival_block: Number(e.target.value) })} />
          </label>
          <label className="field">
            <span>agent detection block</span>
            <input type="number" value={p.agent_detection_block} onChange={(e) => setP({ agent_detection_block: Number(e.target.value) })} />
          </label>
          <label className="field">
            <span>legit single / batch claims per block</span>
            <div className="row" style={{ margin: 0 }}>
              <input type="number" value={p.legit_single_claims_per_block} onChange={(e) => setP({ legit_single_claims_per_block: Number(e.target.value) })} style={{ width: 70 }} />
              <input type="number" value={p.legit_batch_claims_per_block} onChange={(e) => setP({ legit_batch_claims_per_block: Number(e.target.value) })} style={{ width: 70 }} />
            </div>
          </label>
          <label className="field">
            <span>exits per block</span>
            <input type="number" value={p.exits_per_block} onChange={(e) => setP({ exits_per_block: Number(e.target.value) })} />
          </label>
          <label className="field">
            <span>integrations affected by a pause</span>
            <input type="number" value={p.integrations_affected_by_pause} onChange={(e) => setP({ integrations_affected_by_pause: Number(e.target.value) })} />
          </label>
          <label className="field">
            <span>the diagnosis is wrong (false positive)</span>
            <input type="checkbox" checked={p.false_positive} onChange={(e) => setP({ false_positive: e.target.checked })} /> <span className="small">no exploit; shows the cost of premature action</span>
          </label>
          <div className="field">
            <span className="kicker">&nbsp;</span>
            <button className="btn primary" disabled={busy} onClick={() => act(() => api.incidentSetParams("D", p))}>
              recompute all policies
            </button>
          </div>
        </div>
      </Section>

      <Section title="Consequences of waiting and of acting" kicker="every policy, same parameters · click a row for its timeline">
        <table>
          <thead>
            <tr>
              <th>policy</th>
              <th className="num">loss until contained</th>
              <th className="num">loss after a lapse</th>
              <th>containment</th>
              <th className="num">single claims interrupted</th>
              <th className="num">batch claims</th>
              <th className="num">exits blocked</th>
              <th className="num">integrations</th>
              <th className="num">fix at block</th>
            </tr>
          </thead>
          <tbody>
            {inc.outcomes.map((o) => (
              <tr key={o.policy.id} className={`clickable ${o.policy.id === (selected ?? inc.policy_id) ? "decision" : ""}`} onClick={() => setSelected(o.policy.id)}>
                <td>
                  {o.policy.label}
                  {o.policy.is_default && <Pill kind="neutral">default</Pill>}
                </td>
                <td className="num">{fmt(o.loss_before_containment)}</td>
                <td className="num">{fmt(o.loss_after_containment)}</td>
                <td className="small">{o.containment_action ? `${o.containment_action.replace(/_/g, " ")} at block ${o.containment_block}` : "none"}</td>
                <td className="num">{fmt(o.interrupted.single_claims)}</td>
                <td className="num">{fmt(o.interrupted.batch_claims)}</td>
                <td className="num">{fmt(o.interrupted.exits)}</td>
                <td className="num">{fmt(o.interrupted.integrations)}</td>
                <td className="num">{fmt(o.fix_block)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {shown && (
          <div className="card quiet">
            <div className="kicker">
              {shown.policy.label} · {shown.false_positive ? "false positive case" : "exploit in progress"}
            </div>
            <div className="small" style={{ marginBottom: 6 }}>
              side effects: {shown.side_effects}
            </div>
            <div className="timeline">
              {shown.timeline.map((t, i) => (
                <div key={i}>
                  <span className="b">block {t.block}</span>
                  <span className="mono">{t.actor}</span>
                  <span>{t.event}</span>
                </div>
              ))}
            </div>
            <ul className="plain small muted" style={{ marginTop: 8 }}>
              {shown.authority_notes.map((n, i) => (
                <li key={i}>{n}</li>
              ))}
            </ul>
          </div>
        )}
      </Section>

      <Section title="Test the limits on the live model" kicker="the agent asks; the engine decides under the selected policy">
        <div className="row">
          {ws.rungs.map((r) => (
            <button key={r.rung} className="btn" disabled={busy} title={r.side_effects} onClick={() => request(r.rung)}>
              agent: {r.label}
            </button>
          ))}
        </div>
        <div className="row">
          <button
            className="btn danger"
            disabled={busy}
            onClick={async () => {
              const res = await run(() => api.incidentAgentLift("D"));
              if (res) {
                setMessage(res.message);
                setScenario(res.scenario);
              }
            }}
          >
            agent: lift containment
          </button>
          <button className="btn ochre" disabled={busy} onClick={() => act(() => api.incidentGuardiansLift("D"))}>
            guardians: lift containment
          </button>
          <button className="btn" disabled={busy} onClick={() => act(() => api.advanceBlocks("D", 250))}>
            advance 250 blocks
          </button>
          <button className="btn" disabled={busy} onClick={() => act(() => api.incidentClose("D"))}>
            guardians: close incident
          </button>
          <button
            className="btn"
            onClick={() => {
              setActive("D");
              navigate({ page: "scenario", key: "D" });
            }}
          >
            prepare the fix (Stage A) →
          </button>
        </div>
        {message && <Notice kind={message.startsWith("Refused") ? "bad" : "ok"}>{message}</Notice>}
        <div className="small">
          live model: block <span className="mono">{fmt(v.state.block)}</span> · pause <span className="mono">{v.state.pause_detail}</span>
          {inc.grant && (
            <span>
              {" "}
              · grant <span className="mono">{inc.grant.rung.replace(/_/g, " ")}</span> from block {inc.grant.granted_at_block}
              {inc.grant.expires_at_block !== null ? ` to ${inc.grant.expires_at_block}` : ""} {inc.grant.lifted_by_guardians ? "(lifted by guardians)" : ""}
            </span>
          )}
        </div>
        <details>
          <summary>incident log ({inc.log.length})</summary>
          <ul className="plain small">
            {inc.log.map((l, i) => (
              <li key={i}>{l}</li>
            ))}
          </ul>
        </details>
      </Section>
    </div>
  );
}
