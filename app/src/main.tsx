import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

if (navigator.userAgent.includes("Mac")) {
  document.documentElement.dataset.os = "mac";
}

async function boot() {
  // Mock bridge first so ?mock=1 works without the Tauri runtime.
  if (new URLSearchParams(window.location.search).has("mock")) {
    await import("./mock");
  }
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>
  );
}

boot();
