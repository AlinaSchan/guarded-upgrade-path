// Platform helpers for the desktop shell: native dialogs, revealing exported
// files and opening the one external link in the system browser. The plugin
// modules are loaded on first use so that a plain browser tab (the Vite dev
// server) can still render the desktop-only notice.

/** True inside the Tauri desktop shell; false in a plain browser tab. */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Native folder picker; null when cancelled. */
export async function chooseDirectory(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const dir = await open({ directory: true, multiple: false, title: "Choose a folder for the verification bundle" });
  return typeof dir === "string" ? dir : null;
}

/** Native save dialog for Markdown exports; null when cancelled. */
export async function chooseSavePath(defaultName: string): Promise<string | null> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  const p = await save({ defaultPath: defaultName, filters: [{ name: "Markdown", extensions: ["md"] }] });
  return p ?? null;
}

export async function revealPath(path: string): Promise<void> {
  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
  await revealItemInDir(path);
}

/** Opens an https link in the system browser; the shell itself never navigates. */
export async function openExternal(url: string): Promise<void> {
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}
