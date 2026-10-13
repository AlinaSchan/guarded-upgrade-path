// The UI talks to one `api` object. In the desktop shell it forwards to Tauri
// commands; in the browser build it forwards to the WebAssembly engine. The
// backend is chosen once, at startup; nothing in the UI decides authority.

import { tauriApi, type Api } from "./backend";
import { isTauri } from "./platform";

export type { Api };

const backend: Promise<Api> = isTauri() ? Promise.resolve(tauriApi) : import("./wasmApi").then((m) => m.createWasmApi());

/** Resolves once the engine is ready (immediately on the desktop). */
export const backendReady: Promise<void> = backend.then(() => undefined);

export const api = Object.fromEntries(
  Object.keys(tauriApi).map((k) => [
    k,
    async (...args: unknown[]) => {
      const b = await backend;
      const fn = b[k as keyof Api] as unknown as (...a: unknown[]) => Promise<unknown>;
      return fn(...args);
    },
  ]),
) as unknown as Api;

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}
