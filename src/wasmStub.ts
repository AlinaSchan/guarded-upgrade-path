// Stand-in for the WebAssembly engine in builds that do not include it (the
// desktop shell talks to Tauri commands instead). See vite.config.ts.

const MESSAGE = "The browser engine is not part of this build";

export default async function init(): Promise<never> {
  throw new Error(MESSAGE);
}

export class WasmApp {
  constructor() {
    throw new Error(MESSAGE);
  }
  static load(): never {
    throw new Error(MESSAGE);
  }
}
