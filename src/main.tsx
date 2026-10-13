import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { DesktopOnly } from "./components/DesktopOnly";
import { isTauri } from "./platform";
import "@fontsource-variable/public-sans";
import "@fontsource/fira-code/400.css";
import "@fontsource/fira-code/500.css";
import "@fontsource/fira-code/600.css";
import "./styles.css";

// The interface exists only inside the Windows/macOS desktop shell. Opened in a
// plain browser tab (for example the Vite dev server) it shows a notice instead.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isTauri() ? <App /> : <DesktopOnly />}</React.StrictMode>,
);
