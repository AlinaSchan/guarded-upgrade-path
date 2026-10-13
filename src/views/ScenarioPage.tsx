import { useState } from "react";
import { api } from "../api";
import { ScenarioPicker, useApp } from "../App";
import { Diff, fmt, Hash, Kv, Notice, OpenQuestion, pct, Pill, Section } from "../components/common";
import { ChecksTable, EvidenceTable } from "../components/EvidenceTable";
import type { AgentStep, ReplayStats, ScenarioView } from "../types";

const STEPS: { step: AgentStep; label: string }[] = [
  { step: "observe", label: "1 observe" },
  { step: "detect", label: "2 detect" },
  { step: "diagnose", label: "3 diagnose" },
  { step: "construct", label: "4 construct" },
  { step: "check", label: "5 checks" },
  { step: "simulate", label: "6 simulate" },
  { step: "assemble", label: "7 assemble" },
  { step: "commit", label: "8 commit H" },
  { step: "submit", label: "9 submit" },
];

const ORDER = ["idle", "observed", "detected", "diagnosed", "constructed", "checked", "simulated", "assembled", "committed", "submitted"];

function stepState(v: ScenarioView, i: number): "done" | "next" | "" {
  const pos = ORDER.indexOf(v.status);
  if (pos < 0) return "done";
  if (i < pos) return "done";
  if (i === pos) return "next";
  return "";
}

function ReplayCard({ title, r }: { title: string; r: ReplayStats }) {
  return (
    <div className="card quiet">
      <div className="kicker">{title}</div>
      <Kv
        rows={[
          ["version", <span className="mono">{r.version} · {r.impl_id}</span>],
          ["blocks", <span className="mono">{fmt(r.first_block)}–{fmt(r.last_block)}</span>],
          ["claims", `${fmt(r.claims)} (${fmt(r.paid)} paid, ${fmt(r.rejected)} rejected, ${fmt(r.misreads)} misreads)`],
          ["duplicate payouts", <span>{fmt(r.duplicate_count)} to {fmt(r.affected_wallets)} wallets</span>],
          ["gross / net / fee", `${fmt(r.total_gross)} / ${fmt(r.total_net)} / ${fmt(r.total_fee)} units`],
          ["overpayment", r.overpayment_gross ? `${fmt(r.overpayment_gross)} units ≈ ${pct(r.overpayment_share_bps)} of average daily distribution on affected days` : "none"],
          ["migrated entries", fmt(r.migrated_entries)],
        ]}
      />
      {r.duplicates.length > 0 && (
        <details>
          <summary>duplicate payouts ({r.duplicates.length} shown)</summary>
          <table>
            <thead>
              <tr>
                <th>wallet</th>
                <th>period</th>
                <th>first block</th>
                <th>second block</th>
                <th className="num">amount</th>
              </tr>
            </thead>
            <tbody>
              {r.duplicates.map((d, i) => (
                <tr key={i}>
                  <td className="mono">{d.wallet}</td>
                  <td className="mono">{d.block_period}</td>
                  <td className="mono">{d.first_block}</td>
                  <td className="mono">{d.second_block}</td>
                  <td className="num">{fmt(d.amount_gross)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </details>
      )}
    </div>
  );
}

export function ScenarioPage() {
  const { scenarios, active, run, setScenario, navigate, busy } = useApp();
  const v = scenarios[active];
  const [variant, setVariant] = useState<"default" | "flat_amount">("default");
  if (!v) return <p className="muted">Loading case {active}…</p>;
  const next = v.next_agent_step;
  const act = (fn: () => Promise<ScenarioView>) => run(fn, setScenario);

  return (
    <div className="page">
      <div className="row">
        <ScenarioPicker />
        <span className="kicker" style={{ margin: 0 }}>
          case {v.key} · {v.meta.package_title} · proposal {v.proposal_id} revision {v.revision}
        </span>
      </div>
      <h1>
        {v.key} {v.meta.title}
      </h1>
      <div className="row small">
        {v.meta.tags.map((t) => (
          <span key={t.label}>
            <span className="mono muted">{t.label}</span> <span className="mono">{t.value}</span>
          </span>
        ))}
      </div>
      <div className="grid-2">
        <div>
          <div className="kicker">technical change</div>
          <p>{v.meta.technical_change}</p>
        </div>
        <div>
          <div className="kicker">governance effect</div>
          <p>{v.meta.governance_effect}</p>
        </div>
      </div>
      <OpenQuestion>{v.meta.open_question}</OpenQuestion>

      <Section title="Stage A · Proposal (autonomous agent)" kicker="the agent holds no key that changes protocol state">
        <div className="stepper">
          {STEPS.map((s, i) => (
            <div key={s.step} className={`step ${stepState(v, i)}`}>
              {s.label}
            </div>
          ))}
        </div>
        <div className="toolbar">
          <span className="mono muted">status: {v.status_label}</span>
          {next && (
            <button className="btn primary" disabled={busy} onClick={() => act(() => api.agentStep(v.key, next))}>
              agent: {next}
            </button>
          )}
          {next && next !== "revise" && (
            <button className="btn" disabled={busy} onClick={() => act(() => api.agentRun(v.key))}>
              run to submission
            </button>
          )}
          {v.status === "changes_requested" && (
            <button className="btn ochre" disabled={busy} onClick={() => act(() => api.agentStep(v.key, "revise"))}>
              revise (new H, new approval)
            </button>
          )}
          <button className="btn" disabled={busy} onClick={() => act(() => api.resetScenario(v.key))}>
            reset case {v.key}
          </button>
          {v.stage !== "proposal" && (
            <button className="btn" onClick={() => navigate({ page: v.stage === "approval" ? "approval" : "execution", key: v.key })}>
              go to {v.stage} stage →
            </button>
          )}
        </div>
        {v.key === "A" && !v.candidate && (
          <div className="card quiet">
            <div className="kicker">candidate variant (for failure mode 1)</div>
            <div className="row">
              <select
                value={variant}
                onChange={(e) => {
                  const val = e.target.value as "default" | "flat_amount";
                  setVariant(val);
                  act(() => api.setCandidateVariant(v.key, val));
                }}
              >
                <option value="default">2.3.1 — the fix as drafted</option>
                <option value="flat_amount">2.3.1-flat — passes the checked properties, pays a flat amount</option>
              </select>
              <span className="small muted">The second variant shows a proof that holds for a property that omits the payout amount.</span>
            </div>
          </div>
        )}
        {v.completed.length > 0 && (
          <Notice>
            Earlier proposals in this case:{" "}
            {v.completed.map((c) => (
              <span key={c.proposal_id} className="mono">
                {c.proposal_id} ({c.status}) {c.commitment ? <Hash hex={c.commitment} /> : null};{" "}
              </span>
            ))}
          </Notice>
        )}
      </Section>

      <Section title="Protocol state (local model)">
        <Kv
          rows={[
            ["block / period", <span className="mono">{fmt(v.state.block)} / {v.state.period}</span>],
            ["Distribution", <span className="mono">{v.state.version} · state format {v.state.format_version} · feeBps {v.state.fee_bps}</span>],
            ["payouts on record", `${fmt(v.state.payout_count)} (${fmt(v.state.duplicates)} duplicates)`],
            ["treasury / fee collected", `${fmt(v.state.treasury)} / ${fmt(v.state.fee_collected)} units`],
            ["pause", v.state.paused ? <Pill kind="ochre">{v.state.pause_detail}</Pill> : <span className="muted">not paused</span>],
            ["installed functions", <span className="mono small">{v.state.functions.join(" · ")}</span>],
          ]}
        />
      </Section>

      {v.observation && (
        <Section title="Observe" kicker="step 1">
          <table>
            <thead>
              <tr>
                <th>signal</th>
                <th>value</th>
                <th>note</th>
              </tr>
            </thead>
            <tbody>
              {v.observation.signals.map((s) => (
                <tr key={s.name}>
                  <td className="rowlabel">{s.name}</td>
                  <td className="mono">{s.value}</td>
                  <td className="small muted">{s.note}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      )}

      {v.detection && (
        <Section title={`Detect · ${v.detection.title}`} kicker="step 2 · exact query over the local model's log">
          <pre>{v.detection.query}</pre>
          <p>{v.detection.result}</p>
          <div className="small mono muted">
            block range {fmt(v.detection.block_range[0])}–{fmt(v.detection.block_range[1])}
          </div>
          {v.detection.rows.length > 0 && (
            <details>
              <summary>result rows ({v.detection.rows.length})</summary>
              <pre>{v.detection.rows.join("\n")}</pre>
            </details>
          )}
        </Section>
      )}

      {v.diagnosis && (
        <Section title="Diagnose" kicker="step 3">
          <p>{v.diagnosis}</p>
        </Section>
      )}

      {v.candidate && (
        <Section title="Candidate artifact" kicker="step 4 · manifest + payload; H is computed from these two and nothing else">
          <Kv
            rows={[
              ["package", <span className="mono">{v.candidate.manifest.package} {v.candidate.manifest.from_version} → {v.candidate.manifest.to_version} ({v.candidate.manifest.change_kind.replace(/_/g, " ")})</span>],
              ["summary", v.candidate.manifest.summary],
              ["payload", <span className="mono">{fmt(v.candidate.payload_len)} bytes · sha256 {v.candidate.payload_sha256.slice(0, 16)}…</span>],
              ["H (recomputed now)", <Hash hex={v.candidate.commitment} full />],
              ["produced by", v.candidate.manifest.produced_by],
            ]}
          />
          {v.tamper && (
            <Notice kind="bad">
              A substitution is in effect: {v.tamper.description} The agent's interface still shows <Hash hex={v.tamper.original_commitment} />.
            </Notice>
          )}
          {v.candidate_diff && (
            <details open>
              <summary>diff against the installed implementation</summary>
              <Diff text={v.candidate_diff} />
            </details>
          )}
          <details>
            <summary>manifest (JSON)</summary>
            <pre>{v.candidate.manifest_json}</pre>
          </details>
          <details>
            <summary>payload (implementation bundle, canonical JSON — deterministic example data standing in for bytecode)</summary>
            <pre>{v.candidate.payload_text}</pre>
          </details>
        </Section>
      )}

      {v.checks.length > 0 && (
        <Section title="Deterministic checks" kicker="step 5 · run on the parsed payload, never on the summary">
          <ChecksTable checks={v.checks} />
        </Section>
      )}

      {v.replay && (
        <Section title="Simulation (deterministic local replay)" kicker="step 6 · establishes only what happened in the covered cases">
          <p>{v.replay.narrative}</p>
          <div className="small muted">{v.replay.stream_description}</div>
          <div className="grid-2">
            <ReplayCard title={`installed · ${v.replay.production.version}`} r={v.replay.production} />
            <ReplayCard title={`candidate · ${v.replay.candidate.version}`} r={v.replay.candidate} />
          </div>
          <div className="grid-2">
            {[v.replay.fuzz_current, v.replay.fuzz_candidate].map((f, i) => (
              <div key={i} className="card quiet">
                <div className="kicker">fuzzed property · {f.version}</div>
                <div>{f.property}</div>
                <div className="mono small">
                  {fmt(f.runs)} runs · {fmt(f.claims)} claims · {fmt(f.violations)} violations in {f.runs_with_violation} runs
                </div>
                <div className="small muted">{f.model}</div>
              </div>
            ))}
          </div>
          <div className="small muted">Per-wallet totals differ between the two replays for {fmt(v.replay.differing_wallets)} wallets.</div>
        </Section>
      )}

      {v.economics && (
        <Section title="Illustrative economic effect" kicker="case B · who receives what shifts; no invariant settles whether that is desirable">
          <table>
            <thead>
              <tr>
                <th>group</th>
                <th className="num">net at {v.economics.parameter} {v.economics.before}</th>
                <th className="num">net at {v.economics.parameter} {v.economics.after}</th>
                <th className="num">delta</th>
                <th className="num">delta</th>
              </tr>
            </thead>
            <tbody>
              {v.economics.rows.map((r) => (
                <tr key={r.group}>
                  <td>{r.group}</td>
                  <td className="num">{fmt(r.before)}</td>
                  <td className="num">{fmt(r.after)}</td>
                  <td className="num">{r.delta > 0 ? "+" : ""}{fmt(r.delta)}</td>
                  <td className="num">{(r.delta_bps / 100).toFixed(2)}%</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="small">{v.economics.note}</p>
          <div className="small mono muted">{fmt(v.economics.transfers_under_new_value)} transfers would be made under the new value over {v.economics.periods} periods; reverting the value does not reverse them.</div>
        </Section>
      )}

      {v.package && (
        <Section title="Upgrade evidence package" kicker={`step 7–8 · ${v.package.id} · claimed H ${v.package.claimed_commitment.slice(0, 10)}… · each row tagged by how it is known`}>
          <p>
            Guardians approve a change to the protocol, not a report about it. Each row is tagged by how it is known, so that what was proven, what was observed and what is left to judgment
            stay apart. Click a row for its method, scope and what it does not cover.
          </p>
          <EvidenceTable pkg={v.package} />
          {v.displayed_summary && (
            <div className="card">
              <div className="kicker">what the agent's own interface renders (not an input to any verification)</div>
              <div>
                <strong>{v.displayed_summary.title}</strong> — {v.displayed_summary.summary}
              </div>
              <ul className="plain">
                {v.displayed_summary.lines.map((l, i) => (
                  <li key={i}>{l}</li>
                ))}
              </ul>
              <div className="small">
                displayed H: <Hash hex={v.displayed_summary.claimed_commitment} full /> <span className="muted">· rendered by {v.displayed_summary.rendered_by}</span>
              </div>
            </div>
          )}
          <div className="row">
            <button className="btn" onClick={() => navigate({ page: "approval", key: v.key })}>
              open the approval surface →
            </button>
          </div>
        </Section>
      )}

      {v.narrative.length > 0 && (
        <Section title="Narrative" kicker="what the engine did, in order">
          <ul className="plain small">
            {v.narrative.map((l, i) => (
              <li key={i}>{l}</li>
            ))}
          </ul>
        </Section>
      )}
    </div>
  );
}
