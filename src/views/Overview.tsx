import { ScenarioPicker, useApp } from "../App";
import { Notice, OpenQuestion, Section } from "../components/common";
import type { ProposalStatus } from "../types";

type DStep = { id: string; name: string; desc: string; filled: boolean; handoff?: string };

const PROPOSAL: DStep[] = [
  { id: "observe", name: "Observe", desc: "chain state, events, monitoring signals", filled: false },
  { id: "propose", name: "Propose", desc: "detect a condition, diagnose it, construct a candidate upgrade", filled: false },
  { id: "verify", name: "Verify", desc: "check invariants, simulate on a replay", filled: false },
  { id: "evidence", name: "Evidence", desc: "package results, commit to hash H", filled: true, handoff: "handoff: evidence package + hash H" },
];
const APPROVAL: DStep[] = [
  { id: "review", name: "Review", desc: "read the package, recompute H independently of the proposer", filled: false },
  { id: "quorum", name: "Quorum", desc: "m of n sign H, reject it, or request changes", filled: true, handoff: "handoff: signatures over H" },
];
const EXECUTION: DStep[] = [
  { id: "apply", name: "Apply approved artifact", desc: "H and nothing else", filled: true },
  { id: "post", name: "Verify post-state", desc: "expected invariants and state hold: continue, or escalate to guardians", filled: false },
];

function currentStep(status: ProposalStatus | undefined): string | null {
  switch (status) {
    case "observed":
      return "observe";
    case "detected":
    case "diagnosed":
    case "constructed":
      return "propose";
    case "checked":
    case "simulated":
      return "verify";
    case "assembled":
    case "committed":
      return "evidence";
    case "submitted":
    case "under_review":
    case "changes_requested":
      return "review";
    case "approved":
    case "rejected":
      return "quorum";
    case "execution_refused":
      return "apply";
    case "executed":
    case "escalated":
      return "post";
    default:
      return null;
  }
}

function Steps({ steps, current }: { steps: DStep[]; current: string | null }) {
  return (
    <div className="steps">
      {steps.map((s) => (
        <div key={s.id}>
          <div className={`dstep ${s.filled ? "filled" : ""} ${current === s.id ? "current" : ""}`}>
            <div className="name">{s.name}</div>
            <div className="desc">{s.desc}</div>
          </div>
          {s.handoff && <div className="handoff">{s.handoff}</div>}
        </div>
      ))}
    </div>
  );
}

export function Overview() {
  const { ws, scenarios, active, navigate } = useApp();
  const inst = scenarios[active];
  const current = currentStep(inst?.status);
  if (!ws) return null;
  return (
    <div className="page">
      <div className="kicker">review draft · page 1</div>
      <h1>Guarded Upgrade Path</h1>
      <p className="lead">Where autonomous upgrade preparation should stop and human protocol authority should begin.</p>
      <p>
        An agent can take an upgrade a long way on its own: notice a condition, build a candidate, check it against invariants, simulate it and package the results. This sandbox tests where that
        work should stop. The working hypothesis is that preparation can be delegated while the decision to change the protocol cannot, and that the decision only holds if the payload executed
        is the one the guardians reviewed. Machine evidence can show that a candidate meets its specification. Whether the protocol should accept the change is a separate judgment, and the
        cases in this program look at where the two come apart.
      </p>
      <Notice kind="ochre">
        This hypothesis is <strong>under test</strong>, not presented as an established or universally safe governance model. Its assumptions are listed below; the six failure modes and the five
        questions for review are where it is meant to be pushed.
      </Notice>

      <div className="card quiet">
        <div className="kicker">how to use this program (about 15 minutes)</div>
        <ol className="plain" style={{ paddingLeft: 22 }}>
          <li>
            Open <strong>case A</strong> and press <em>run to submission</em>; read the evidence package and click rows to see what each check does not cover.
          </li>
          <li>
            In <strong>Approval surface &amp; guardians</strong>, compare modes A/B/C, recompute H, and sign as two guardians.
          </li>
          <li>
            In <strong>Execution &amp; verification</strong>, execute; then substitute the payload and execute again to see the refusal; run the verifier or export a bundle for the CLI.
          </li>
          <li>
            Try <strong>case B</strong> (economics stays a judgment), <strong>case C</strong> with <em>users act</em> and a code revert (blocked), and <strong>case D</strong> under different incident policies.
          </li>
          <li>
            Run the six failure modes, then write notes against the five <strong>questions for review</strong> and export them.
          </li>
        </ol>
        <div className="row">
          <button className="btn small" onClick={() => navigate({ page: "scenario", key: "A" })}>
            start with case A
          </button>
          <button className="btn small" onClick={() => navigate({ page: "scope" })}>
            what is simulated
          </button>
        </div>
      </div>

      <div className="row">
        <span className="kicker" style={{ margin: 0 }}>
          diagram follows the active case:
        </span>
        <ScenarioPicker />
        {inst && (
          <span className="mono muted">
            {inst.meta.package_title} · {inst.status_label}
          </span>
        )}
      </div>

      <div className="diagram">
        <div className="stage">
          <div className="stage-name">Proposal</div>
          <div className="actor">agent</div>
          <div className="small">May observe, build a candidate, run checks and assemble evidence. Holds no key that changes protocol state.</div>
        </div>
        <div className="stage">
          <Steps steps={PROPOSAL} current={current} />
        </div>
        <div className="stage legend">
          <div>
            <span className="sw" /> authority changes hands
          </div>
          <div>
            <span className="sw band" /> decided by guardians
          </div>
          <div>
            <span className="sw dash" /> open question, not assumed
          </div>
          <div>
            <span className="sw dot" /> step (filled: produces a handoff)
          </div>
        </div>

        <div className="stage approval">
          <div className="stage-name">Approval</div>
          <div className="actor">guardian quorum</div>
          <div className="small">Decides whether the protocol may change. Approves one artifact, by its hash, or none.</div>
          <div className="small" style={{ marginTop: 8, fontStyle: "italic", color: "var(--ochre-2)" }}>
            Dashed: a shortcut past quorum. Not assumed. Explored as explicitly hypothetical policies in case D.
          </div>
        </div>
        <div className="stage approval">
          <Steps steps={APPROVAL} current={current} />
        </div>
        <div className="stage approval small" style={{ fontStyle: "italic" }}>
          Unexpected post-state returns to the guardians: recovery is a new decision, not a rollback.
        </div>

        <div className="stage">
          <div className="stage-name">Execution</div>
          <div className="actor">executor</div>
          <div className="small">Applies only the approved artifact and reports the result. No discretion of its own.</div>
        </div>
        <div className="stage">
          <Steps steps={EXECUTION} current={current} />
        </div>
        <div className="stage" />
      </div>

      <Section title="The hypothesis, stated as claims">
        <ul className="plain">
          {ws.hypothesis.map((h, i) => (
            <li key={i}>{h}</li>
          ))}
        </ul>
      </Section>

      <Section title="What it assumes, and where each assumption is tested">
        <ul className="plain">
          {ws.assumptions.map((a, i) => (
            <li key={i}>{a}</li>
          ))}
        </ul>
      </Section>

      <Section title="Four changes, four different risks" kicker="review draft · page 3">
        <p>The size of a change and the weight of the decision behind it are separate measures. Each case pairs what changes in the code with what changes for the protocol.</p>
        <div className="grid-2">
          {ws.scenarios.map((s) => {
            const v = scenarios[s.key];
            return (
              <div key={s.key} className="card">
                <div className="kicker">
                  {s.key} · {s.package_title}
                </div>
                <h3 style={{ marginTop: 0 }}>{s.title}</h3>
                {v && (
                  <div className="row small">
                    {v.meta.tags.map((t) => (
                      <span key={t.label}>
                        <span className="mono muted">{t.label}</span> <span className="mono">{t.value}</span>
                      </span>
                    ))}
                  </div>
                )}
                {v && <OpenQuestion>{v.meta.open_question}</OpenQuestion>}
                <div className="row">
                  <span className="mono muted">{s.status_label}</span>
                  <button className="btn small" onClick={() => navigate({ page: "scenario", key: s.key })}>
                    open case {s.key}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </Section>

      <Section title="Unresolved questions">
        <p>
          The five questions on page 4 of the draft are reproduced under <em>Questions for review</em>. They stay unresolved in this program; nothing here answers them on the reviewer's behalf.
        </p>
        <button className="btn" onClick={() => navigate({ page: "questions" })}>
          open the questions
        </button>
        <button className="btn" onClick={() => navigate({ page: "failures" })}>
          where the model can fail
        </button>
        <button className="btn" onClick={() => navigate({ page: "lab" })}>
          Review Lab
        </button>
      </Section>
    </div>
  );
}
