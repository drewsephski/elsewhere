import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toaster } from "@/components/ui/sonner";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <TooltipProvider>
      <App />
      <Toaster position="top-center" richColors closeButton />
    </TooltipProvider>
  </React.StrictMode>,
);
