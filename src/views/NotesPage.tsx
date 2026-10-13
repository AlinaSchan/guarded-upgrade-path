import { useEffect, useState } from "react";
import { chooseSavePath } from "../platform";
import { api } from "../api";
import { useApp } from "../App";
import { Notice, Section } from "../components/common";
import type { ReviewNote } from "../types";

const EMPTY: ReviewNote = { id: 0, created_seq: 0, observed_problem: "", scenario: "", technical_concern: "", suggested_policy_change: "", remaining_question: "", question_ref: null, created_at: null };

export function NotesPage() {
  const { ws, run, busy, route } = useApp();
  const [draft, setDraft] = useState<ReviewNote>({ ...EMPTY, question_ref: route.question ?? null });
  const [preview, setPreview] = useState<string | null>(null);
  const [savedTo, setSavedTo] = useState<string | null>(null);

  useEffect(() => {
    if (route.question) setDraft((d) => ({ ...d, question_ref: route.question ?? null }));
  }, [route.question]);

  if (!ws) return null;
  const set = (patch: Partial<ReviewNote>) => setDraft({ ...draft, ...patch });
  const editing = draft.id !== 0;

  const submit = async () => {
    const fn = editing ? () => api.updateNote(draft) : () => api.addNote(draft);
    const r = await run(fn);
    if (r) setDraft({ ...EMPTY });
  };

  const exportMd = async () => {
    const path = await chooseSavePath("guarded-upgrade-path-review-notes.md");
    if (!path) return;
    const written = await run(() => api.exportNotesMarkdown(path));
    if (written) setSavedTo(written);
  };

  return (
    <div className="page">
      <h1>Review notes</h1>
      <p className="lead">Local notes for the reviewer. Stored on this machine only; exported as Markdown on request. Nothing here is presented as already supplied or approved.</p>
      <Section title={editing ? `Edit note ${draft.id}` : "New note"}>
        <div className="grid-2">
          <label className="field">
            <span>relevant scenario</span>
            <select value={draft.scenario} onChange={(e) => set({ scenario: e.target.value })}>
              <option value="">—</option>
              {ws.scenarios.map((s) => (
                <option key={s.key} value={`Case ${s.key} — ${s.title}`}>
                  Case {s.key} — {s.title}
                </option>
              ))}
              <option value="Approval surface">Approval surface</option>
              <option value="Independent verification">Independent verification</option>
              <option value="Recovery model">Recovery model</option>
              <option value="Incident authority">Incident authority</option>
              <option value="Failure modes">Failure modes</option>
              <option value="General">General</option>
            </select>
          </label>
          <label className="field">
            <span>question for review (optional)</span>
            <select value={draft.question_ref ?? ""} onChange={(e) => set({ question_ref: e.target.value ? Number(e.target.value) : null })}>
              <option value="">—</option>
              {ws.questions.map((q) => (
                <option key={q.number} value={q.number}>
                  {q.number} · {q.concerns}
                </option>
              ))}
            </select>
          </label>
        </div>
        <label className="field">
          <span>observed problem</span>
          <textarea value={draft.observed_problem} onChange={(e) => set({ observed_problem: e.target.value })} />
        </label>
        <label className="field">
          <span>technical concern</span>
          <textarea value={draft.technical_concern} onChange={(e) => set({ technical_concern: e.target.value })} />
        </label>
        <label className="field">
          <span>suggested policy change</span>
          <textarea value={draft.suggested_policy_change} onChange={(e) => set({ suggested_policy_change: e.target.value })} />
        </label>
        <label className="field">
          <span>remaining question</span>
          <textarea value={draft.remaining_question} onChange={(e) => set({ remaining_question: e.target.value })} />
        </label>
        <div className="toolbar">
          <button className="btn primary" disabled={busy} onClick={submit}>
            {editing ? "save changes" : "save note"}
          </button>
          {editing && (
            <button className="btn" onClick={() => setDraft({ ...EMPTY })}>
              cancel
            </button>
          )}
          <button className="btn" disabled={busy || ws.notes.length === 0} onClick={exportMd}>
            export notes as Markdown
          </button>
          <button
            className="btn"
            disabled={busy}
            onClick={async () => {
              const md = await run(() => api.notesMarkdownPreview());
              if (md) setPreview(md);
            }}
          >
            preview Markdown
          </button>
        </div>
        {savedTo && <Notice kind="ok">Notes written to <span className="mono">{savedTo}</span></Notice>}
      </Section>
      <Section title={`Saved notes (${ws.notes.length})`}>
        {ws.notes.length === 0 && <p className="muted">No notes yet.</p>}
        {ws.notes.map((n) => (
          <div key={n.id} className="card">
            <div className="row" style={{ justifyContent: "space-between" }}>
              <span className="kicker" style={{ margin: 0 }}>
                note {n.id} · {n.created_at ?? ""} · {n.scenario || "—"}
                {n.question_ref ? ` · question ${n.question_ref}` : ""}
              </span>
              <span>
                <button className="btn small" onClick={() => setDraft(n)}>
                  edit
                </button>
                <button className="btn small danger" disabled={busy} onClick={() => run(() => api.deleteNote(n.id))}>
                  delete
                </button>
              </span>
            </div>
            {[
              ["observed problem", n.observed_problem],
              ["technical concern", n.technical_concern],
              ["suggested policy change", n.suggested_policy_change],
              ["remaining question", n.remaining_question],
            ].map(([k, v]) => (
              <div key={k} className="small" style={{ marginTop: 4 }}>
                <span className="mono muted">{k}: </span>
                {v || <span className="muted">—</span>}
              </div>
            ))}
          </div>
        ))}
      </Section>
      {preview && (
        <Section title="Markdown preview">
          <pre style={{ maxHeight: 420 }}>{preview}</pre>
        </Section>
      )}
    </div>
  );
}
