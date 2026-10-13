// Type surface of the wasm-bindgen output of crates/gup-wasm (src/wasm/gup_wasm.js),
// aliased as "@gup-wasm" (vite.config.ts, tsconfig.json). Every method mirrors a
// desktop command and returns JSON text.

declare module "@gup-wasm" {
  export default function init(input?: RequestInfo | URL | Response | BufferSource | WebAssembly.Module): Promise<unknown>;

  export class WasmApp {
    constructor();
    static load(json: string): WasmApp;
    free(): void;
    to_json(): string;
    stamp(now: string): void;
    get_workspace(): string;
    get_scenario(key: string): string;
    agent_step(key: string, step: string): string;
    agent_run(key: string): string;
    set_candidate_variant(key: string, variant: string): string;
    reset_scenario(key: string): string;
    reset_all(): string;
    get_surface(key: string, mode: string): string;
    compare_surfaces(key: string): string;
    set_surface_mode(key: string, mode: string): string;
    guardian_inspect(key: string): string;
    guardian_recompute(key: string): string;
    guardian_decide(key: string, guardianId: string, decision: string, note: string): string;
    set_quorum_policy(key: string, policyId: string, m: number, n: number): string;
    tamper(key: string, variant: string): string;
    restore_candidate(key: string): string;
    execute(key: string): string;
    verify_independently(key: string): string;
    verification_bundle(key: string): string;
    assess_recovery(key: string): string;
    simulate_users_act(key: string, periods: number): string;
    attempt_code_revert(key: string): string;
    draft_recovery(key: string): string;
    incident_set_policy(key: string, policyId: string): string;
    incident_set_params(key: string, paramsJson: string): string;
    incident_agent_request(key: string, rung: string): string;
    incident_agent_lift(key: string): string;
    incident_guardians_lift(key: string): string;
    incident_close(key: string): string;
    advance_blocks(key: string, blocks: number): string;
    run_failure_mode(key: string): string;
    run_all_failure_modes(): string;
    add_note(noteJson: string): string;
    update_note(noteJson: string): string;
    delete_note(id: number): string;
    notes_markdown(): string;
    observations_markdown(): string;
    record_export(what: string, name: string): void;
    set_dark_mode(enabled: boolean): boolean;
  }
}
