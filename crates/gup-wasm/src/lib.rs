//! Browser binding of the domain engine.
//!
//! The web build runs the same Rust engine as the desktop application,
//! compiled to WebAssembly. Every method mirrors a desktop command and
//! exchanges JSON strings; the JavaScript side persists the workspace in
//! `localStorage` and supplies timestamps. Nothing here decides anything
//! that the engine does not decide.

use gup_domain::engine::{AgentStep, CandidateVariant, TamperVariant, Workspace};
use gup_domain::exports::observations_markdown;
use gup_domain::failure_modes::{self, FailureModeId};
use gup_domain::guardians::Decision;
use gup_domain::incident::{IncidentParams, Rung};
use gup_domain::notes::{notes_markdown, ReviewNote};
use gup_domain::surface::SurfaceMode;
use gup_domain::views::{scenario_view, workspace_view};
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmApp {
    ws: Workspace,
}

fn js<T: Serialize>(v: &T) -> Result<String, JsError> {
    serde_json::to_string(v).map_err(|e| JsError::new(&e.to_string()))
}

fn parse<T: DeserializeOwned>(json: &str) -> Result<T, JsError> {
    serde_json::from_str(json).map_err(|e| JsError::new(&e.to_string()))
}

/// Parse a plain enum token such as `observe` into its serde representation.
fn token<T: DeserializeOwned>(s: &str) -> Result<T, JsError> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|e| JsError::new(&e.to_string()))
}

fn err<E: std::fmt::Display>(e: E) -> JsError {
    JsError::new(&e.to_string())
}

#[wasm_bindgen]
impl WasmApp {
    /// A fresh workspace built from the deterministic fixtures.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmApp {
        WasmApp {
            ws: Workspace::new(),
        }
    }

    /// Restore a workspace persisted by `to_json`. Falls back to a fresh one
    /// if the stored version no longer matches.
    pub fn load(json: &str) -> WasmApp {
        match Workspace::from_json(json) {
            Ok(ws) => WasmApp { ws },
            Err(_) => WasmApp {
                ws: Workspace::new(),
            },
        }
    }

    pub fn to_json(&self) -> Result<String, JsError> {
        self.ws.to_json().map_err(|e| JsError::new(&e))
    }

    /// Fill in missing timestamps (the engine itself has no clock).
    pub fn stamp(&mut self, now: &str) {
        for e in self.ws.audit.iter_mut().filter(|e| e.timestamp.is_none()) {
            e.timestamp = Some(now.to_string());
        }
        for n in self.ws.notes.iter_mut().filter(|n| n.created_at.is_none()) {
            n.created_at = Some(now.to_string());
        }
    }

    pub fn get_workspace(&self) -> Result<String, JsError> {
        js(&workspace_view(&self.ws))
    }

    pub fn get_scenario(&self, key: &str) -> Result<String, JsError> {
        js(&scenario_view(self.ws.scenario(key).map_err(err)?))
    }

    fn scenario_json(&self, key: &str) -> Result<String, JsError> {
        self.get_scenario(key)
    }

    pub fn agent_step(&mut self, key: &str, step: &str) -> Result<String, JsError> {
        let step: AgentStep = token(step)?;
        self.ws.agent_step(key, step).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn agent_run(&mut self, key: &str) -> Result<String, JsError> {
        self.ws.agent_run_to_submission(key).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn set_candidate_variant(&mut self, key: &str, variant: &str) -> Result<String, JsError> {
        let v: CandidateVariant = token(variant)?;
        self.ws.set_candidate_variant(key, v).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn reset_scenario(&mut self, key: &str) -> Result<String, JsError> {
        self.ws.reset_scenario(key).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn reset_all(&mut self) -> Result<String, JsError> {
        self.ws.reset_all();
        self.get_workspace()
    }

    pub fn get_surface(&self, key: &str, mode: &str) -> Result<String, JsError> {
        let m: SurfaceMode = token(mode)?;
        js(&self.ws.surface(key, m).map_err(err)?)
    }

    pub fn compare_surfaces(&self, key: &str) -> Result<String, JsError> {
        js(&self.ws.compare_surfaces(key).map_err(err)?)
    }

    pub fn set_surface_mode(&mut self, key: &str, mode: &str) -> Result<String, JsError> {
        let m: SurfaceMode = token(mode)?;
        self.ws.set_surface_mode(key, m).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn guardian_inspect(&mut self, key: &str) -> Result<String, JsError> {
        self.ws.guardian_inspect(key).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn guardian_recompute(&mut self, key: &str) -> Result<String, JsError> {
        js(&self.ws.guardian_recompute(key).map_err(err)?)
    }

    pub fn guardian_decide(
        &mut self,
        key: &str,
        guardian_id: &str,
        decision: &str,
        note: &str,
    ) -> Result<String, JsError> {
        let d: Decision = token(decision)?;
        js(&self
            .ws
            .guardian_decide(key, guardian_id, d, note)
            .map_err(err)?)
    }

    /// `m` and `n` of 0 mean "use the preset named by policy_id".
    pub fn set_quorum_policy(
        &mut self,
        key: &str,
        policy_id: &str,
        m: u32,
        n: u32,
    ) -> Result<String, JsError> {
        let custom = if m > 0 && n > 0 {
            Some((m as usize, n as usize))
        } else {
            None
        };
        self.ws
            .set_quorum_policy(key, policy_id, custom)
            .map_err(err)?;
        self.scenario_json(key)
    }

    pub fn tamper(&mut self, key: &str, variant: &str) -> Result<String, JsError> {
        let v: TamperVariant = token(variant)?;
        self.ws.tamper(key, v).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn restore_candidate(&mut self, key: &str) -> Result<String, JsError> {
        self.ws.restore_candidate(key).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn execute(&mut self, key: &str) -> Result<String, JsError> {
        self.ws.execute(key).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn verify_independently(&self, key: &str) -> Result<String, JsError> {
        js(&self.ws.verify_independently(key).map_err(err)?)
    }

    /// The verification bundle as a JSON array of `{name, bytes_base64}`.
    pub fn verification_bundle(&mut self, key: &str) -> Result<String, JsError> {
        let files = self.ws.verification_bundle(key).map_err(err)?;
        let entries: Vec<serde_json::Value> = files
            .iter()
            .map(|(name, bytes)| serde_json::json!({ "name": name, "bytes_base64": base64_encode(bytes) }))
            .collect();
        self.ws.audit(
            Some(key),
            "reviewer",
            "export verification bundle",
            format!("{} files downloaded from the browser build", files.len()),
            None,
        );
        js(&entries)
    }

    pub fn assess_recovery(&mut self, key: &str) -> Result<String, JsError> {
        self.ws.assess_recovery(key).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn simulate_users_act(&mut self, key: &str, periods: u32) -> Result<String, JsError> {
        self.ws
            .simulate_users_act(key, periods as u64)
            .map_err(err)?;
        self.scenario_json(key)
    }

    pub fn attempt_code_revert(&mut self, key: &str) -> Result<String, JsError> {
        self.ws.attempt_code_revert(key).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn draft_recovery(&mut self, key: &str) -> Result<String, JsError> {
        self.ws.draft_recovery(key).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn incident_set_policy(&mut self, key: &str, policy_id: &str) -> Result<String, JsError> {
        self.ws.incident_set_policy(key, policy_id).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn incident_set_params(&mut self, key: &str, params_json: &str) -> Result<String, JsError> {
        let p: IncidentParams = parse(params_json)?;
        self.ws.incident_set_params(key, p).map_err(err)?;
        self.scenario_json(key)
    }

    /// Returns `{ok, message, scenario}` like the desktop command.
    pub fn incident_agent_request(&mut self, key: &str, rung: &str) -> Result<String, JsError> {
        let r: Rung = token(rung)?;
        let (ok, message) = match self.ws.incident_agent_request(key, r) {
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
        let scenario = scenario_view(self.ws.scenario(key).map_err(err)?);
        js(&serde_json::json!({ "ok": ok, "message": message, "scenario": scenario }))
    }

    pub fn incident_agent_lift(&mut self, key: &str) -> Result<String, JsError> {
        let (ok, message) = match self.ws.incident_agent_lift(key) {
            Ok(()) => (true, "Lifted".to_string()),
            Err(e) => (false, format!("Refused: {e}")),
        };
        let scenario = scenario_view(self.ws.scenario(key).map_err(err)?);
        js(&serde_json::json!({ "ok": ok, "message": message, "scenario": scenario }))
    }

    pub fn incident_guardians_lift(&mut self, key: &str) -> Result<String, JsError> {
        self.ws.incident_guardians_lift(key).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn incident_close(&mut self, key: &str) -> Result<String, JsError> {
        self.ws.incident_close(key).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn advance_blocks(&mut self, key: &str, blocks: u32) -> Result<String, JsError> {
        self.ws.advance_blocks(key, blocks as u64).map_err(err)?;
        self.scenario_json(key)
    }

    pub fn run_failure_mode(&mut self, key: &str) -> Result<String, JsError> {
        let id = FailureModeId::parse(key)
            .ok_or_else(|| JsError::new(&format!("unknown failure mode {key}")))?;
        let result = failure_modes::run(id);
        self.ws
            .failure_results
            .insert(result.key.clone(), result.clone());
        self.ws.audit(
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
        js(&result)
    }

    pub fn run_all_failure_modes(&mut self) -> Result<String, JsError> {
        let results = failure_modes::run_all();
        for r in &results {
            self.ws.failure_results.insert(r.key.clone(), r.clone());
        }
        self.ws.audit(
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
        js(&results)
    }

    pub fn add_note(&mut self, note_json: &str) -> Result<String, JsError> {
        let n: ReviewNote = parse(note_json)?;
        self.ws.add_note(n);
        js(&self.ws.notes)
    }

    pub fn update_note(&mut self, note_json: &str) -> Result<String, JsError> {
        let n: ReviewNote = parse(note_json)?;
        self.ws.update_note(n);
        js(&self.ws.notes)
    }

    pub fn delete_note(&mut self, id: u32) -> Result<String, JsError> {
        self.ws.delete_note(id as u64);
        js(&self.ws.notes)
    }

    pub fn notes_markdown(&self) -> String {
        notes_markdown(&self.ws.notes)
    }

    pub fn observations_markdown(&self) -> String {
        observations_markdown(&self.ws)
    }

    pub fn record_export(&mut self, what: &str, name: &str) {
        self.ws.audit(
            None,
            "reviewer",
            what,
            format!("{name} downloaded from the browser build"),
            None,
        );
    }

    pub fn set_dark_mode(&mut self, enabled: bool) -> bool {
        self.ws.settings.dark_mode = enabled;
        enabled
    }
}

impl Default for WasmApp {
    fn default() -> Self {
        Self::new()
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip_and_full_path() {
        let mut app = WasmApp::new();
        let ws: serde_json::Value = serde_json::from_str(&app.get_workspace().unwrap()).unwrap();
        assert_eq!(ws["scenarios"].as_array().unwrap().len(), 4);
        app.agent_run("A").unwrap();
        app.guardian_decide("A", "guardian-1", "approve", "")
            .unwrap();
        app.guardian_decide("A", "guardian-2", "approve", "")
            .unwrap();
        let s: serde_json::Value = serde_json::from_str(&app.execute("A").unwrap()).unwrap();
        assert_eq!(s["status"], "executed");
        app.stamp("2026-10-13T00:00:00Z");
        let json = app.to_json().unwrap();
        let again = WasmApp::load(&json);
        let s2: serde_json::Value =
            serde_json::from_str(&again.get_scenario("A").unwrap()).unwrap();
        assert_eq!(s2["status"], "executed");
        let bundle: serde_json::Value =
            serde_json::from_str(&WasmApp::load(&json).verification_bundle("A").unwrap()).unwrap();
        assert!(bundle.as_array().unwrap().len() >= 5);
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
    }
}
