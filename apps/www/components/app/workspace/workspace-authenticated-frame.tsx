"use client";

import { ProductThemeScope } from "@/components/app/product-theme-scope";
import { Toaster } from "@/components/ui/sonner";
import { useDesktopNativeNotifications } from "@/hooks/use-desktop-native-notifications";
import type { ReactNode } from "react";
import { WorkspaceAppLayout } from "./workspace-app-layout";

/**
 * Signed-in workspace chrome shared by Next (`app/app/layout.tsx`) and Tauri
 * (`src/cloud-shell.tsx`). Chat UI is not rendered here — `WorkspaceAppLayout`
 * mounts `WorkspaceShell`, which hosts `BotConversationView` / group chat from
 * the URL. Child routes should mount {@link WorkspaceRouteOutlet} only.
 */
export function WorkspaceAuthenticatedFrame({
  userEmail,
  children,
}: {
  userEmail: string;
  children: ReactNode;
}) {
  useDesktopNativeNotifications();

  return (
    <>
      <ProductThemeScope />
      <WorkspaceAppLayout userEmail={userEmail}>{children}</WorkspaceAppLayout>
      <Toaster position="bottom-right" />
    </>
  );
}
