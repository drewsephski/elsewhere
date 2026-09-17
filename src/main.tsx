import React from "react";
import ReactDOM from "react-dom/client";
import DesktopApp from "@desktop/surfaces/desktop/desktop-app";
import { ThemeProvider } from "@desktop/components/theme-provider";
import { MotionIconConfig } from "@desktop/components/icons/lucide";
import { TooltipProvider } from "@desktop/components/ui/tooltip";
import { Toaster } from "@desktop/components/ui/sonner";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider attribute="class" defaultTheme="light" enableSystem={false}>
      <MotionIconConfig trigger="hover" mode="signature" duration={0.35}>
        <TooltipProvider>
          <DesktopApp />
          <Toaster position="top-center" richColors closeButton />
        </TooltipProvider>
      </MotionIconConfig>
    </ThemeProvider>
  </React.StrictMode>,
);
