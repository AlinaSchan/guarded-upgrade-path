// The UI talks to one `api` object that forwards to Tauri commands. Nothing
// in the UI decides authority: every answer comes from the Rust engine.

import { tauriApi, type Api } from "./backend";

export type { Api };

export const api: Api = tauriApi;

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}
