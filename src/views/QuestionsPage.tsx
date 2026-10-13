import { useApp } from "../App";
import type { Page } from "../App";
import { Notice, Pill } from "../components/common";

export function QuestionsPage() {
  const { ws, navigate, route } = useApp();
  if (!ws) return null;
  return (
    <div className="page">
      <div className="kicker">review draft · page 4 · verbatim</div>
      <h1>Questions for Bertrand</h1>
      <p className="lead">These five questions are unresolved. The program links each one to the interactive scenario that bears on it; it does not answer them, and nothing here should be read as an answer already supplied.</p>
      {ws.questions.map((q) => (
        <div key={q.number} className={`card ${route.question === q.number ? "band" : ""}`}>
          <div className="row" style={{ justifyContent: "space-between" }}>
            <span className="kicker" style={{ margin: 0 }}>
              question {q.number} · {q.concerns}
            </span>
            <Pill kind="ochre">{q.status}</Pill>
          </div>
          <p style={{ fontSize: 17 }}>
            <span style={{ color: "var(--ochre-2)", fontFamily: "var(--mono)", marginRight: 10 }}>{q.number}</span>
            {q.text}
          </p>
          <div className="row">
            {q.links.map((l) => (
              <button key={l.label} className="btn small" onClick={() => navigate({ page: l.route as Page, key: l.scenario ?? undefined, failure: l.failure_mode ?? undefined })}>
                {l.label} →
              </button>
            ))}
            <button className="btn small ochre" onClick={() => navigate({ page: "notes", question: q.number })}>
              write a note for question {q.number}
            </button>
          </div>
        </div>
      ))}
      <Notice>
        Proposed review scope (from the draft): if the topic is relevant and both sides agree terms, a short written review of the proposal, approval and execution split; the four upgrade cases; the evidence
        package and its tags; the line between technical verification and governance approval; emergency authority; rollback versus recovery; and failure modes the draft misses. Based on general professional
        judgment and public systems only. Final scope and compensation are agreed before substantive review begins.
      </Notice>
    </div>
  );
}
