// Browser backend: the Rust engine compiled to WebAssembly, state persisted in
// this browser's localStorage, exports delivered as downloads.

import init, { WasmApp } from "@gup-wasm";
import type { Api } from "./backend";
import { basename, downloadBytes } from "./platform";
import type { ExportResult, ReviewNote } from "./types";
import { base64ToBytes, zipStore } from "./zip";

export const STORAGE_KEY = "gup.workspace.v4";

function safeGet(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function safeSet(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Private windows or blocked storage: the session simply does not persist.
  }
}

export async function createWasmApi(): Promise<Api> {
  await init();
  const saved = safeGet(STORAGE_KEY);
  const app: WasmApp = saved ? WasmApp.load(saved) : new WasmApp();

  const persist = () => {
    app.stamp(new Date().toISOString());
    safeSet(STORAGE_KEY, app.to_json());
  };
  const j = <T>(s: string): T => JSON.parse(s) as T;
  const mut = <T>(s: string): T => {
    persist();
    return j<T>(s);
  };
  persist();

  const download = (what: string, name: string, text: string): string => {
    downloadBytes(name, text, "text/markdown");
    app.record_export(what, name);
    persist();
    return name;
  };

  const api: Api = {
    workspace: async () => j(app.get_workspace()),
    scenario: async (key) => j(app.get_scenario(key)),
    agentStep: async (key, step) => mut(app.agent_step(key, step)),
    agentRun: async (key) => mut(app.agent_run(key)),
    setCandidateVariant: async (key, variant) => mut(app.set_candidate_variant(key, variant)),
    resetScenario: async (key) => mut(app.reset_scenario(key)),
    resetAll: async () => mut(app.reset_all()),
    surface: async (key, mode) => j(app.get_surface(key, mode)),
    compareSurfaces: async (key) => j(app.compare_surfaces(key)),
    setSurfaceMode: async (key, mode) => mut(app.set_surface_mode(key, mode)),
    guardianInspect: async (key) => mut(app.guardian_inspect(key)),
    guardianRecompute: async (key) => mut(app.guardian_recompute(key)),
    guardianDecide: async (key, guardianId, decision, note) => mut(app.guardian_decide(key, guardianId, decision, note)),
    setQuorumPolicy: async (key, policyId, m, n) => mut(app.set_quorum_policy(key, policyId, m ?? 0, n ?? 0)),
    tamper: async (key, variant) => mut(app.tamper(key, variant)),
    restoreCandidate: async (key) => mut(app.restore_candidate(key)),
    execute: async (key) => mut(app.execute(key)),
    verifyIndependently: async (key) => j(app.verify_independently(key)),
    exportVerificationBundle: async (key): Promise<ExportResult> => {
      const entries = j<{ name: string; bytes_base64: string }[]>(app.verification_bundle(key));
      persist();
      const files = entries.map((e) => ({ name: e.name, data: base64ToBytes(e.bytes_base64) }));
      const name = `gup-bundle-${key.toLowerCase()}.zip`;
      downloadBytes(name, zipStore(files), "application/zip");
      return { directory: `downloaded as ${name}`, files: files.map((f) => f.name) };
    },
    assessRecovery: async (key) => mut(app.assess_recovery(key)),
    simulateUsersAct: async (key, periods) => mut(app.simulate_users_act(key, periods)),
    attemptCodeRevert: async (key) => mut(app.attempt_code_revert(key)),
    draftRecovery: async (key) => mut(app.draft_recovery(key)),
    incidentSetPolicy: async (key, policyId) => mut(app.incident_set_policy(key, policyId)),
    incidentSetParams: async (key, params) => mut(app.incident_set_params(key, JSON.stringify(params))),
    incidentAgentRequest: async (key, rung) => mut(app.incident_agent_request(key, rung)),
    incidentAgentLift: async (key) => mut(app.incident_agent_lift(key)),
    incidentGuardiansLift: async (key) => mut(app.incident_guardians_lift(key)),
    incidentClose: async (key) => mut(app.incident_close(key)),
    advanceBlocks: async (key, blocks) => mut(app.advance_blocks(key, blocks)),
    runFailureMode: async (key) => mut(app.run_failure_mode(key)),
    runAllFailureModes: async () => mut(app.run_all_failure_modes()),
    addNote: async (note: ReviewNote) => mut(app.add_note(JSON.stringify(note))),
    updateNote: async (note: ReviewNote) => mut(app.update_note(JSON.stringify(note))),
    deleteNote: async (id) => mut(app.delete_note(id)),
    notesMarkdownPreview: async () => app.notes_markdown(),
    exportNotesMarkdown: async (path) => download("export notes", basename(path) || "review-notes.md", app.notes_markdown()),
    exportObservationsMarkdown: async (path) => download("export observations", basename(path) || "review-observations.md", app.observations_markdown()),
    observationsMarkdownPreview: async () => app.observations_markdown(),
    setDarkMode: async (enabled) => {
      const r = app.set_dark_mode(enabled);
      persist();
      return r;
    },
    dataDir: async () => `this browser's localStorage (key ${STORAGE_KEY}); nothing leaves the device`,
  };
  return api;
}
