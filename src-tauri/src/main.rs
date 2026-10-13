//! Guarded Upgrade Path — desktop shell.
//!
//! The frontend only asks. Every decision (which transitions are allowed,
//! whether an authorization is valid, whether a payload matches H) is made by
//! `gup-domain`. This file wires commands, persistence and exports.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gup_domain::engine::{
    AgentStep, CandidateVariant, GuardianOutcome, RecomputeResult, TamperVariant, Workspace,
};
use gup_domain::exports::observations_markdown;
use gup_domain::failure_modes::{self, FailureModeId, FailureModeResult};
use gup_domain::guardians::Decision;
use gup_domain::incident::{IncidentParams, Rung};
use gup_domain::notes::{notes_markdown, ReviewNote};
use gup_domain::surface::{SurfaceComparison, SurfaceMode, SurfaceView};
use gup_domain::verifier::VerifierReport;
use gup_domain::views::{scenario_view, workspace_view, ScenarioView, WorkspaceView};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Manager, State};

struct AppState {
    ws: Mutex<Workspace>,
    data_dir: PathBuf,
    audit_saved: Mutex<u64>,
}

/// UTC timestamp without external crates (Howard Hinnant's civil-from-days).
fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3_600,
        (rem % 3_600) / 60,
        rem % 60
    )
}

impl AppState {
    fn load(data_dir: PathBuf) -> AppState {
        let path = data_dir.join("workspace.json");
        let loaded = fs::read_to_string(&path)
            .ok()
            .and_then(|j| Workspace::from_json(&j).ok());
        let (ws, saved) = match loaded {
            Some(ws) => {
                let saved = ws.audit.iter().map(|e| e.seq).max().unwrap_or(0);
                (ws, saved)
            }
            None => (Workspace::new(), 0),
        };
        let state = AppState {
            ws: Mutex::new(ws),
            data_dir,
            audit_saved: Mutex::new(saved),
        };
        if let Ok(mut ws) = state.ws.lock() {
            let _ = state.persist(&mut ws);
        }
        state
    }

    fn persist(&self, ws: &mut Workspace) -> Result<(), String> {
        let stamp = now_iso();
        for e in ws.audit.iter_mut().filter(|e| e.timestamp.is_none()) {
            e.timestamp = Some(stamp.clone());
        }
        for n in ws.notes.iter_mut().filter(|n| n.created_at.is_none()) {
            n.created_at = Some(stamp.clone());
        }
        fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        let json = ws.to_json()?;
        let tmp = self.data_dir.join("workspace.json.tmp");
        let path = self.data_dir.join("workspace.json");
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &path)
            .or_else(|_| fs::copy(&tmp, &path).map(|_| ()))
            .map_err(|e| e.to_string())?;
        let mut saved = self
            .audit_saved
            .lock()
            .map_err(|_| "audit lock poisoned".to_string())?;
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.data_dir.join("audit.jsonl"))
            .map_err(|e| e.to_string())?;
        for e in ws.audit.iter().filter(|e| e.seq > *saved) {
            let line = serde_json::to_string(e).map_err(|e| e.to_string())?;
            writeln!(f, "{line}").map_err(|e| e.to_string())?;
        }
        *saved = ws.audit.iter().map(|e| e.seq).max().unwrap_or(*saved);
        Ok(())
    }
}

fn mutate<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&mut Workspace) -> Result<T, String>,
) -> Result<T, String> {
    let mut ws = state
        .ws
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    let out = f(&mut ws)?;
    state.persist(&mut ws)?;
    Ok(out)
}

fn read<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&Workspace) -> Result<T, String>,
) -> Result<T, String> {
    let ws = state
        .ws
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    f(&ws)
}

fn view(ws: &Workspace, key: &str) -> Result<ScenarioView, String> {
    ws.scenario(key)
        .map(scenario_view)
        .map_err(|e| e.to_string())
}

fn write_text(path: &str, text: &str) -> Result<String, String> {
    let p = Path::new(path);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(p, text).map_err(|e| e.to_string())?;
    Ok(p.to_string_lossy().into_owned())
}

// ------------------------------------------------------------------ commands

#[tauri::command]
fn get_workspace(state: State<'_, AppState>) -> Result<WorkspaceView, String> {
    read(&state, |ws| Ok(workspace_view(ws)))
}

#[tauri::command]
fn get_scenario(state: State<'_, AppState>, key: String) -> Result<ScenarioView, String> {
    read(&state, |ws| view(ws, &key))
}

#[tauri::command]
fn agent_step(
    state: State<'_, AppState>,
    key: String,
    step: AgentStep,
) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.agent_step(&key, step).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn agent_run(state: State<'_, AppState>, key: String) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.agent_run_to_submission(&key)
            .map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn set_candidate_variant(
    state: State<'_, AppState>,
    key: String,
    variant: CandidateVariant,
) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.set_candidate_variant(&key, variant)
            .map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn reset_scenario(state: State<'_, AppState>, key: String) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.reset_scenario(&key).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn reset_all(state: State<'_, AppState>) -> Result<WorkspaceView, String> {
    mutate(&state, |ws| {
        ws.reset_all();
        Ok(workspace_view(ws))
    })
}

#[tauri::command]
fn get_surface(
    state: State<'_, AppState>,
    key: String,
    mode: SurfaceMode,
) -> Result<SurfaceView, String> {
    read(&state, |ws| {
        ws.surface(&key, mode).map_err(|e| e.to_string())
    })
}

#[tauri::command]
fn compare_surfaces(state: State<'_, AppState>, key: String) -> Result<SurfaceComparison, String> {
    read(&state, |ws| {
        ws.compare_surfaces(&key).map_err(|e| e.to_string())
    })
}

#[tauri::command]
fn set_surface_mode(
    state: State<'_, AppState>,
    key: String,
    mode: SurfaceMode,
) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.set_surface_mode(&key, mode).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn guardian_inspect(state: State<'_, AppState>, key: String) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.guardian_inspect(&key).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn guardian_recompute(state: State<'_, AppState>, key: String) -> Result<RecomputeResult, String> {
    mutate(&state, |ws| {
        ws.guardian_recompute(&key).map_err(|e| e.to_string())
    })
}

#[tauri::command]
fn guardian_decide(
    state: State<'_, AppState>,
    key: String,
    guardian_id: String,
    decision: Decision,
    note: String,
) -> Result<GuardianOutcome, String> {
    mutate(&state, |ws| {
        ws.guardian_decide(&key, &guardian_id, decision, &note)
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
fn set_quorum_policy(
    state: State<'_, AppState>,
    key: String,
    policy_id: String,
    m: Option<usize>,
    n: Option<usize>,
) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        let custom = match (m, n) {
            (Some(m), Some(n)) => Some((m, n)),
            _ => None,
        };
        ws.set_quorum_policy(&key, &policy_id, custom)
            .map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn tamper(
    state: State<'_, AppState>,
    key: String,
    variant: TamperVariant,
) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.tamper(&key, variant).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn restore_candidate(state: State<'_, AppState>, key: String) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.restore_candidate(&key).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn execute(state: State<'_, AppState>, key: String) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.execute(&key).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn verify_independently(state: State<'_, AppState>, key: String) -> Result<VerifierReport, String> {
    read(&state, |ws| {
        ws.verify_independently(&key).map_err(|e| e.to_string())
    })
}

#[derive(Serialize)]
struct ExportResult {
    directory: String,
    files: Vec<String>,
}

#[tauri::command]
fn export_verification_bundle(
    state: State<'_, AppState>,
    key: String,
    directory: String,
) -> Result<ExportResult, String> {
    let files = read(&state, |ws| {
        ws.verification_bundle(&key).map_err(|e| e.to_string())
    })?;
    let dir = PathBuf::from(&directory).join(format!("gup-bundle-{}", key.to_ascii_lowercase()));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut written = Vec::new();
    for (name, bytes) in files {
        let p = dir.join(&name);
        fs::write(&p, bytes).map_err(|e| e.to_string())?;
        written.push(p.to_string_lossy().into_owned());
    }
    mutate(&state, |ws| {
        ws.audit(
            Some(&key),
            "reviewer",
            "export verification bundle",
            format!("{} files written to {}", written.len(), dir.display()),
            None,
        );
        Ok(())
    })?;
    Ok(ExportResult {
        directory: dir.to_string_lossy().into_owned(),
        files: written,
    })
}

#[tauri::command]
fn assess_recovery(state: State<'_, AppState>, key: String) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.assess_recovery(&key).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn simulate_users_act(
    state: State<'_, AppState>,
    key: String,
    periods: u64,
) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.simulate_users_act(&key, periods)
            .map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn attempt_code_revert(state: State<'_, AppState>, key: String) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.attempt_code_revert(&key).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn draft_recovery(state: State<'_, AppState>, key: String) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.draft_recovery(&key).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[derive(Serialize)]
struct ActionResult {
    ok: bool,
    message: String,
    scenario: ScenarioView,
}

#[tauri::command]
fn incident_set_policy(
    state: State<'_, AppState>,
    key: String,
    policy_id: String,
) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.incident_set_policy(&key, &policy_id)
            .map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn incident_set_params(
    state: State<'_, AppState>,
    key: String,
    params: IncidentParams,
) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.incident_set_params(&key, params)
            .map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn incident_agent_request(
    state: State<'_, AppState>,
    key: String,
    rung: Rung,
) -> Result<ActionResult, String> {
    mutate(&state, |ws| {
        let r = ws.incident_agent_request(&key, rung);
        let (ok, message) = match r {
            Ok(g) => (
                true,
                format!(
                    "Granted: {} under {} (expires {})",
                    g.rung.label(),
                    g.policy_id,
                    g.expires_at_block
                        .map(|b| format!("block {b}"))
                        .unwrap_or_else(|| "never; no state change".into())
                ),
            ),
            Err(e) => (false, format!("Refused: {e}")),
        };
        Ok(ActionResult {
            ok,
            message,
            scenario: view(ws, &key)?,
        })
    })
}

#[tauri::command]
fn incident_agent_lift(state: State<'_, AppState>, key: String) -> Result<ActionResult, String> {
    mutate(&state, |ws| {
        let r = ws.incident_agent_lift(&key);
        let (ok, message) = match r {
            Ok(()) => (true, "Lifted".into()),
            Err(e) => (false, format!("Refused: {e}")),
        };
        Ok(ActionResult {
            ok,
            message,
            scenario: view(ws, &key)?,
        })
    })
}

#[tauri::command]
fn incident_guardians_lift(
    state: State<'_, AppState>,
    key: String,
) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.incident_guardians_lift(&key)
            .map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn incident_close(state: State<'_, AppState>, key: String) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.incident_close(&key).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn advance_blocks(
    state: State<'_, AppState>,
    key: String,
    blocks: u64,
) -> Result<ScenarioView, String> {
    mutate(&state, |ws| {
        ws.advance_blocks(&key, blocks).map_err(|e| e.to_string())?;
        view(ws, &key)
    })
}

#[tauri::command]
fn run_failure_mode(state: State<'_, AppState>, key: String) -> Result<FailureModeResult, String> {
    let id = FailureModeId::parse(&key).ok_or_else(|| format!("unknown failure mode {key}"))?;
    let result = failure_modes::run(id);
    mutate(&state, |ws| {
        ws.failure_results
            .insert(result.key.clone(), result.clone());
        ws.audit(
            Some(&failure_modes::info(id).scenario),
            "engine",
            "failure mode run",
            format!(
                "{}: expected behaviour {}",
                result.key,
                if result.holds {
                    "observed"
                } else {
                    "NOT observed"
                }
            ),
            None,
        );
        Ok(())
    })?;
    Ok(result)
}

#[tauri::command]
fn run_all_failure_modes(state: State<'_, AppState>) -> Result<Vec<FailureModeResult>, String> {
    let results = failure_modes::run_all();
    mutate(&state, |ws| {
        for r in &results {
            ws.failure_results.insert(r.key.clone(), r.clone());
        }
        ws.audit(
            None,
            "engine",
            "all failure modes run",
            format!(
                "{} of {} held",
                results.iter().filter(|r| r.holds).count(),
                results.len()
            ),
            None,
        );
        Ok(())
    })?;
    Ok(results)
}

#[tauri::command]
fn add_note(state: State<'_, AppState>, note: ReviewNote) -> Result<Vec<ReviewNote>, String> {
    mutate(&state, |ws| {
        ws.add_note(note);
        Ok(ws.notes.clone())
    })
}

#[tauri::command]
fn update_note(state: State<'_, AppState>, note: ReviewNote) -> Result<Vec<ReviewNote>, String> {
    mutate(&state, |ws| {
        ws.update_note(note);
        Ok(ws.notes.clone())
    })
}

#[tauri::command]
fn delete_note(state: State<'_, AppState>, id: u64) -> Result<Vec<ReviewNote>, String> {
    mutate(&state, |ws| {
        ws.delete_note(id);
        Ok(ws.notes.clone())
    })
}

#[tauri::command]
fn notes_markdown_preview(state: State<'_, AppState>) -> Result<String, String> {
    read(&state, |ws| Ok(notes_markdown(&ws.notes)))
}

#[tauri::command]
fn export_notes_markdown(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let md = read(&state, |ws| Ok(notes_markdown(&ws.notes)))?;
    let written = write_text(&path, &md)?;
    mutate(&state, |ws| {
        ws.audit(None, "reviewer", "export notes", written.clone(), None);
        Ok(())
    })?;
    Ok(written)
}

#[tauri::command]
fn export_observations_markdown(
    state: State<'_, AppState>,
    path: String,
) -> Result<String, String> {
    let md = read(&state, |ws| Ok(observations_markdown(ws)))?;
    let written = write_text(&path, &md)?;
    mutate(&state, |ws| {
        ws.audit(
            None,
            "reviewer",
            "export observations",
            written.clone(),
            None,
        );
        Ok(())
    })?;
    Ok(written)
}

#[tauri::command]
fn observations_markdown_preview(state: State<'_, AppState>) -> Result<String, String> {
    read(&state, |ws| Ok(observations_markdown(ws)))
}

#[tauri::command]
fn set_dark_mode(state: State<'_, AppState>, enabled: bool) -> Result<bool, String> {
    mutate(&state, |ws| {
        ws.settings.dark_mode = enabled;
        Ok(enabled)
    })
}

#[tauri::command]
fn data_dir(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.data_dir.to_string_lossy().into_owned())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            fs::create_dir_all(&dir)?;
            app.manage(AppState::load(dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_workspace,
            get_scenario,
            agent_step,
            agent_run,
            set_candidate_variant,
            reset_scenario,
            reset_all,
            get_surface,
            compare_surfaces,
            set_surface_mode,
            guardian_inspect,
            guardian_recompute,
            guardian_decide,
            set_quorum_policy,
            tamper,
            restore_candidate,
            execute,
            verify_independently,
            export_verification_bundle,
            assess_recovery,
            simulate_users_act,
            attempt_code_revert,
            draft_recovery,
            incident_set_policy,
            incident_set_params,
            incident_agent_request,
            incident_agent_lift,
            incident_guardians_lift,
            incident_close,
            advance_blocks,
            run_failure_mode,
            run_all_failure_modes,
            add_note,
            update_note,
            delete_note,
            notes_markdown_preview,
            export_notes_markdown,
            export_observations_markdown,
            observations_markdown_preview,
            set_dark_mode,
            data_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Guarded Upgrade Path review sandbox");
}
