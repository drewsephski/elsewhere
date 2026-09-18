import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { markTauriDocument } from "@/lib/tauri-runtime";
import CloudShell from "./cloud-shell";
import "./shell-globals.css";
import "../apps/www/app/globals.css";

markTauriDocument();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BrowserRouter>
      <CloudShell />
    </BrowserRouter>
  </React.StrictMode>,
);
