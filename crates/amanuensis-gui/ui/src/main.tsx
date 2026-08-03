import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { STORAGE_KEYS } from "./lib/constants";

// Apply saved theme before first render to prevent flash.
// Mirrors the store's setTheme logic: "dark" is the base :root palette (no attribute),
// every other theme is applied via data-theme. Must stay in sync with the Theme union
// so v2 themes (dark-v2/light-v2/midnight-v2) survive a reload.
const savedTheme = localStorage.getItem(STORAGE_KEYS.THEME);
if (savedTheme && savedTheme !== "dark") {
  document.documentElement.dataset.theme = savedTheme;
}

// Suppress the webview's own context menu in production builds.
//
// In a dev build Tauri enables devtools, so right-clicking anywhere shows WKWebView's
// "Reload / Inspect Element" menu. Release builds don't enable the `devtools` feature so
// that menu shouldn't appear, but the webview can still offer its default menu — and
// where we render our own menus (creature names) we only preventDefault on those exact
// elements. This makes the app's own menus the only ones users ever see.
//
// Text inputs and non-empty selections keep the native menu so copy/paste still works.
if (import.meta.env.PROD) {
  document.addEventListener("contextmenu", (e) => {
    const target = e.target as HTMLElement | null;
    const editable =
      target?.closest("input, textarea, [contenteditable='true']") != null;
    const hasSelection = (window.getSelection()?.toString().length ?? 0) > 0;
    if (!editable && !hasSelection) {
      e.preventDefault();
    }
  });
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
