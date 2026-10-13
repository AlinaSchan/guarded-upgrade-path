// Platform helpers: the same UI runs inside the Tauri desktop shell and as a
// browser build (WebAssembly engine, GitHub Pages). Native dialogs exist only
// in the desktop shell; the browser build downloads files instead.

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Desktop: native folder picker. Browser: a marker meaning "download". */
export async function chooseDirectory(): Promise<string | null> {
  if (isTauri()) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const dir = await open({ directory: true, multiple: false, title: "Choose a folder for the verification bundle" });
    return typeof dir === "string" ? dir : null;
  }
  return "download";
}

/** Desktop: native save dialog. Browser: the default file name (downloaded). */
export async function chooseSavePath(defaultName: string): Promise<string | null> {
  if (isTauri()) {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const p = await save({ defaultPath: defaultName, filters: [{ name: "Markdown", extensions: ["md"] }] });
    return p ?? null;
  }
  return defaultName;
}

export async function revealPath(path: string): Promise<void> {
  if (!isTauri()) return;
  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
  await revealItemInDir(path);
}

export function downloadBytes(name: string, data: Uint8Array | string, mime = "application/octet-stream"): void {
  const blob = new Blob([data as BlobPart], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 2000);
}

export function basename(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}
