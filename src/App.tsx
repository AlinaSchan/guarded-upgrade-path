import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { api, errorText } from "./api";
import type { ScenarioView, WorkspaceView } from "./types";
import { Overview } from "./views/Overview";
import { ScenarioPage } from "./views/ScenarioPage";
import { ApprovalPage } from "./views/ApprovalPage";
import { ExecutionPage } from "./views/ExecutionPage";
import { RecoveryPage } from "./views/RecoveryPage";
import { IncidentPage } from "./views/IncidentPage";
import { FailureModesPage } from "./views/FailureModesPage";
import { ReviewLab } from "./views/ReviewLab";
import { QuestionsPage } from "./views/QuestionsPage";
import { NotesPage } from "./views/NotesPage";
import { AuditPage } from "./views/AuditPage";
import { ScopePage } from "./views/ScopePage";

export type Page = "overview" | "scenario" | "approval" | "execution" | "recovery" | "incident" | "failures" | "lab" | "questions" | "notes" | "audit" | "scope";

export interface Route {
  page: Page;
  key?: string;
  failure?: string;
  question?: number;
}

export interface AppCtx {
  ws: WorkspaceView | null;
  scenarios: Record<string, ScenarioView>;
  active: string;
  route: Route;
  busy: boolean;
  error: string | null;
  navigate: (r: Route) => void;
  setActive: (k: string) => void;
  refresh: () => Promise<void>;
  loadScenario: (key: string) => Promise<ScenarioView | undefined>;
  setScenario: (v: ScenarioView) => void;
  run: <T>(fn: () => Promise<T>, after?: (t: T) => void) => Promise<T | undefined>;
  clearError: () => void;
}

const Ctx = createContext<AppCtx | null>(null);

export function useApp(): AppCtx {
  const c = useContext(Ctx);
  if (!c) throw new Error("AppCtx missing");
  return c;
}

const KEYS = ["A", "B", "C", "D"];

export default function App() {
  const [ws, setWs] = useState<WorkspaceView | null>(null);
  const [scenarios, setScenarios] = useState<Record<string, ScenarioView>>({});
  const [route, setRoute] = useState<Route>({ page: "overview" });
  const [active, setActiveState] = useState("A");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setWs(await api.workspace());
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  const setScenario = useCallback((v: ScenarioView) => {
    setScenarios((s) => ({ ...s, [v.key]: v }));
  }, []);

  const loadScenario = useCallback(
    async (key: string) => {
      try {
        const v = await api.scenario(key);
        setScenario(v);
        return v;
      } catch (e) {
        setError(errorText(e));
        return undefined;
      }
    },
    [setScenario],
  );

  const run = useCallback(
    async <T,>(fn: () => Promise<T>, after?: (t: T) => void): Promise<T | undefined> => {
      setBusy(true);
      setError(null);
      try {
        const r = await fn();
        after?.(r);
        await refresh();
        return r;
      } catch (e) {
        setError(errorText(e));
        return undefined;
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  useEffect(() => {
    refresh();
    KEYS.forEach((k) => loadScenario(k));
  }, [refresh, loadScenario]);

  useEffect(() => {
    const dark = ws?.settings.dark_mode ?? false;
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  }, [ws?.settings.dark_mode]);

  const navigate = useCallback((r: Route) => {
    if (r.key) setActiveState(r.key);
    setRoute(r);
    setError(null);
    const el = document.querySelector(".content");
    if (el) el.scrollTop = 0;
  }, []);

  const setActive = useCallback((k: string) => {
    setActiveState(k);
    loadScenario(k);
  }, [loadScenario]);

  const ctx = useMemo<AppCtx>(
    () => ({ ws, scenarios, active, route, busy, error, navigate, setActive, refresh, loadScenario, setScenario, run, clearError: () => setError(null) }),
    [ws, scenarios, active, route, busy, error, navigate, setActive, refresh, loadScenario, setScenario, run],
  );

  const toggleTheme = async () => {
    const next = !(ws?.settings.dark_mode ?? false);
    await run(() => api.setDarkMode(next));
  };

  const nav = (page: Page, label: string, extra?: string, key?: string) => (
    <button key={page + (key ?? "")} className={route.page === page && (key === undefined || route.key === key || (page === "scenario" && active === key)) ? "active" : ""} onClick={() => navigate({ page, key })}>
      <span>{label}</span>
      {extra && <span className="status">{extra}</span>}
    </button>
  );

  return (
    <Ctx.Provider value={ctx}>
      <div className="app">
        <div className="topbar">
          <div>
            <span className="title">Guarded Upgrade Path</span>
            <span className="sub">interactive technical review sandbox · review draft, October 2026</span>
          </div>
          <div className="row" style={{ margin: 0 }}>
            {busy && <span className="mono muted">working…</span>}
            <button className="btn small" onClick={toggleTheme} title="The primary appearance follows the review draft; a dark appearance is optional.">
              {ws?.settings.dark_mode ? "light appearance" : "dark appearance"}
            </button>
          </div>
        </div>
        <nav className="sidebar">
          {nav("overview", "Overview")}
          <div className="group">Upgrade cases</div>
          {ws?.scenarios.map((s) => nav("scenario", `${s.key} · ${s.title}`, s.status_label, s.key))}
          <div className="group">Stages (active case {active})</div>
          {nav("approval", "Approval surface & guardians", scenarios[active]?.stage === "approval" ? "●" : undefined)}
          {nav("execution", "Execution & verification", scenarios[active]?.stage === "execution" ? "●" : undefined)}
          {nav("recovery", "Recovery (not rollback)")}
          {nav("incident", "Active incident (case D)")}
          <div className="group">Review</div>
          {nav("failures", "Where the model can fail", ws ? `${ws.failure_results.length}/6 run` : undefined)}
          {nav("lab", "Review Lab")}
          {nav("questions", "Questions for review")}
          {nav("notes", "Review notes", ws ? String(ws.notes.length) : undefined)}
          {nav("audit", "Audit log", ws ? String(ws.audit.length) : undefined)}
          {nav("scope", "Scope & simulation")}
        </nav>
        <main className="content">
          {error && (
            <div className="notice bad">
              <span className="mono">{error}</span>{" "}
              <button className="btn small" onClick={() => setError(null)}>
                dismiss
              </button>
            </div>
          )}
          {!ws ? (
            <p className="muted">Loading the local workspace…</p>
          ) : (
            <>
              {route.page === "overview" && <Overview />}
              {route.page === "scenario" && <ScenarioPage />}
              {route.page === "approval" && <ApprovalPage />}
              {route.page === "execution" && <ExecutionPage />}
              {route.page === "recovery" && <RecoveryPage />}
              {route.page === "incident" && <IncidentPage />}
              {route.page === "failures" && <FailureModesPage />}
              {route.page === "lab" && <ReviewLab />}
              {route.page === "questions" && <QuestionsPage />}
              {route.page === "notes" && <NotesPage />}
              {route.page === "audit" && <AuditPage />}
              {route.page === "scope" && <ScopePage />}
              <div className="footer">
                <span>Logos & Co · review draft, October 2026 · prepared for Bertrand Juglas · desktop review aid v{ws.app_version}</span>
                <span>offline · deterministic local model · local test keys only</span>
              </div>
            </>
          )}
        </main>
      </div>
    </Ctx.Provider>
  );
}

export function ScenarioPicker() {
  const { ws, active, setActive, navigate, route } = useApp();
  if (!ws) return null;
  return (
    <div className="segmented">
      {ws.scenarios.map((s) => (
        <button
          key={s.key}
          className={active === s.key ? "active" : ""}
          onClick={() => {
            setActive(s.key);
            navigate({ ...route, key: s.key });
          }}
          title={s.package_title}
        >
          {s.key}
        </button>
      ))}
    </div>
  );
}
