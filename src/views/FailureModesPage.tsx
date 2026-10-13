import { api } from "../api";
import { useApp } from "../App";
import { Notice, Pill, Section } from "../components/common";
import type { FailureModeResult } from "../types";

export function FailureModesPage() {
  const { ws, run, navigate, busy, route } = useApp();
  if (!ws) return null;
  const results = new Map(ws.failure_results.map((r) => [r.key, r]));
  const runOne = (key: string) => run(() => api.runFailureMode(key));
  return (
    <div className="page">
      <div className="kicker">review draft · page 4</div>
      <h1>Where the model can fail</h1>
      <p className="lead">Six ways the separation could hold on paper and still fail in practice. Each one runs through the actual domain engine on a fresh workspace, not through a frontend animation, and reports what was expected and what was observed.</p>
      <Notice>
        These scenarios show what the model does. They do not steer the reviewer towards a conclusion; whether the observed behaviour is adequate is exactly the question for review.
        <div className="toolbar" style={{ margin: "8px 0 0" }}>
          <button className="btn primary" disabled={busy} onClick={() => run(() => api.runAllFailureModes())}>
            run all six through the engine
          </button>
        </div>
      </Notice>
      <div className="grid-2">
        {ws.failure_modes.map((f) => {
          const r: FailureModeResult | undefined = results.get(f.key);
          const highlighted = route.failure === f.key;
          return (
            <div key={f.key} className={`card ${highlighted ? "band" : ""}`}>
              <div className="kicker">
                {f.key} · case {f.scenario} · question {f.question}
              </div>
              <h3 style={{ marginTop: 0 }}>{f.title}</h3>
              <p>{f.description}</p>
              {f.reference && <div className="mono small muted">{f.reference}</div>}
              <p className="small" style={{ fontStyle: "italic" }}>
                Needs: {f.needs}
              </p>
              <div className="toolbar">
                <button className="btn" disabled={busy} onClick={() => runOne(f.key)}>
                  run {f.key}
                </button>
                <button className="btn small" onClick={() => navigate({ page: "scenario", key: f.scenario })}>
                  open case {f.scenario}
                </button>
                <button className="btn small" onClick={() => navigate({ page: "questions", question: f.question })}>
                  question {f.question}
                </button>
              </div>
              {r && (
                <div className={`notice ${r.holds ? "ok" : "bad"}`}>
                  <div>
                    expected behaviour <Pill kind={r.holds ? "ok" : "bad"}>{r.holds ? "observed" : "NOT observed"}</Pill>
                  </div>
                  <div className="small" style={{ marginTop: 4 }}>
                    <strong>Expected:</strong> {r.expected}
                  </div>
                  <div className="small">
                    <strong>Observed:</strong> {r.observed}
                  </div>
                  <details>
                    <summary>engine steps ({r.steps.length})</summary>
                    <ul className="plain small">
                      {r.steps.map((s, i) => (
                        <li key={i}>{s}</li>
                      ))}
                    </ul>
                  </details>
                  <details>
                    <summary>structured evidence (JSON)</summary>
                    <pre>{JSON.stringify(r.evidence, null, 2)}</pre>
                  </details>
                </div>
              )}
            </div>
          );
        })}
      </div>
      <Section title="Automated coverage">
        <p className="small">
          Every failure mode is also a Rust test (<span className="mono">gup-domain::failure_modes::tests</span>), and the executor, verifier, quorum, recovery and incident boundaries have their own
          unit tests. The CLI verifier is tested out of process. See docs/PDF_REQUIREMENTS_TRACEABILITY.md.
        </p>
      </Section>
    </div>
  );
}
