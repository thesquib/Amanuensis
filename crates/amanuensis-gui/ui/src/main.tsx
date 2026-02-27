import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { STORAGE_KEYS } from "./lib/constants";

// Apply saved theme before first render to prevent flash
const savedTheme = localStorage.getItem(STORAGE_KEYS.THEME);
if (savedTheme === "light" || savedTheme === "midnight") {
  document.documentElement.dataset.theme = savedTheme;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
