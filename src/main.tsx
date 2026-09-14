import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import CloudShell from "./cloud-shell";
import "../apps/www/app/globals.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BrowserRouter>
      <CloudShell />
    </BrowserRouter>
  </React.StrictMode>,
);
