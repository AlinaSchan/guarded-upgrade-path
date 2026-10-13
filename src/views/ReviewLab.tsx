import { useState } from "react";
import { chooseSavePath } from "../platform";
import { api } from "../api";
import { ScenarioPicker, useApp } from "../App";
import { Notice, Section } from "../components/common";
import type { Page } from "../App";

interface Tool {
  title: string;
  what: string;
  page: Page;
  key?: string;
  action?: () => Promise<unknown>;
  actionLabel?: string;
}

export function ReviewLab() {
  const { ws, scenarios, active, run, setScenario, navigate, busy } = useApp();
  const [preview, setPreview] = useState<string | null>(null);
  const [savedTo, setSavedTo] = useState<string | null>(null);
  if (!ws) return null;
  const v = scenarios[active];

  const exportObservations = async () => {
    const path = await chooseSavePath("guarded-upgrade-path-observations.md");
    if (!path) return;
    const written = await run(() => api.exportObservationsMarkdown(path));
    if (written) setSavedTo(written);
  };

  const tools: Tool[] = [
    { title: "Select one of the four upgrade cases", what: "Cases A–D from page 3; each pairs what changes in code with what changes for the protocol.", page: "scenario", key: active },
    { title: "Compare approval surface modes", what: "Modes A, B and C of the same package, side by side; H identical in every mode.", page: "approval", key: active },
    { title: "Inspect and independently verify H", what: "Recompute H from the artifact as held; run the verifier; export a bundle for the CLI.", page: "execution", key: active },
    { title: "Inspect checked versus assumed properties", what: "Every evidence row tagged checked / simulated / assumed / judgment, with what it does not cover.", page: "scenario", key: active },
    { title: "Test modified payloads", what: "Substitute the artifact after approval and watch the executor refuse it.", page: "execution", key: active },
    { title: "Compare different quorum policies", what: "2-of-3 demo preset, unanimous, the hypothetical lighter path, 3-of-5 and custom m-of-n.", page: "approval", key: active },
    { title: "Examine the economic effect of a parameter change", what: "Case B: feeBps 100 → 250, per-tier net payouts and treasury income.", page: "scenario", key: "B" },
    { title: "Test incident authority limits", what: "Case D: the authority ladder, hypothetical policies, consequences of waiting and of pausing.", page: "incident" },
    { title: "Explore failed execution", what: "Commitment mismatch, invalid authorization, unexpected post-state escalation.", page: "execution", key: active },
    { title: "Distinguish code rollback from recovery", what: "Case C: four layers; automatic revert blocked; recovery as a distinct proposal.", page: "recovery", key: "C" },
    {
      title: "Reset scenarios deterministically",
      what: "Rebuild every case from fixtures. Notes and the audit log are kept.",
      page: "lab",
      action: () => run(() => api.resetAll()),
      actionLabel: "reset all four cases",
    },
    { title: "Export review observations", what: "Markdown of the current state of every case, the failure modes run, the audit tail and the notes.", page: "lab", action: exportObservations, actionLabel: "export observations (.md)" },
  ];

  return (
    <div className="page">
      <h1>Review Lab</h1>
      <p className="lead">The workbench for the technical discussion. Pick a case, then use the tools below; every action runs through the Rust engine and is recorded in the audit log.</p>
      <div className="row">
        <span className="kicker" style={{ margin: 0 }}>
          active case
        </span>
        <ScenarioPicker />
        {v && (
          <span className="mono muted">
            {v.meta.package_title} · {v.status_label}
          </span>
        )}
      </div>
      <Notice>
        Quick path for one case: Stage A <em>run to submission</em> → Stage B <em>recompute H</em> and sign with two guardians → Stage C <em>execute</em>; then try a substitution, a code revert, or an incident policy.
        {v && v.status === "idle" && (
          <button className="btn small" disabled={busy} style={{ marginLeft: 8 }} onClick={() => run(() => api.agentRun(v.key), setScenario)}>
            run case {v.key} to submission now
          </button>
        )}
      </Notice>
      <table>
        <thead>
          <tr>
            <th style={{ width: 260 }}>tool</th>
            <th>what it shows</th>
            <th style={{ width: 200 }}>open</th>
          </tr>
        </thead>
        <tbody>
          {tools.map((t) => (
            <tr key={t.title}>
              <td>{t.title}</td>
              <td className="small">{t.what}</td>
              <td>
                {t.action ? (
                  <button className="btn small" disabled={busy} onClick={() => t.action?.()}>
                    {t.actionLabel}
                  </button>
                ) : (
                  <button className="btn small" onClick={() => navigate({ page: t.page, key: t.key })}>
                    open →
                  </button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {savedTo && <Notice kind="ok">Observations written to <span className="mono">{savedTo}</span></Notice>}
      <Section title="Observations preview">
        <div className="toolbar">
          <button
            className="btn"
            disabled={busy}
            onClick={async () => {
              const md = await run(() => api.observationsMarkdownPreview());
              if (md) setPreview(md);
            }}
          >
            preview the export
          </button>
        </div>
        {preview && <pre style={{ maxHeight: 480 }}>{preview}</pre>}
      </Section>
      <Section title="Local review notes">
        <p className="small">
          Record an observed problem, the relevant scenario, a technical concern, a suggested policy change and a remaining question. Notes stay on this machine and export to Markdown. The application never
          presents them as supplied or approved by anyone but their author.
        </p>
        <button className="btn" onClick={() => navigate({ page: "notes" })}>
          open review notes ({ws.notes.length})
        </button>
      </Section>
    </div>
  );
}
