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

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
