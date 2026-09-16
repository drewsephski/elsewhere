"use client";

import { LegacyAppChrome } from "./legacy-app-chrome";
import { WorkspaceShell } from "./workspace-shell";
import { usePathname } from "next/navigation";

const LEGACY_PREFIXES = [
  "/app/work",
  "/app/approvals",
  "/app/results",
  "/app/routines",
  "/app/computers",
  "/app/connectors",
  "/app/skills",
  "/app/settings",
];

function isLegacyRoute(pathname: string): boolean {
  if (pathname === "/app/bots" && !pathname.includes("/app/bots/")) {
    return true;
  }
  return LEGACY_PREFIXES.some(
    (prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`),
  );
}

interface WorkspaceAppLayoutProps {
  userEmail: string;
  children: React.ReactNode;
}

export function WorkspaceAppLayout({ userEmail, children }: WorkspaceAppLayoutProps) {
  const pathname = usePathname() ?? "/app";

  if (isLegacyRoute(pathname)) {
    return <LegacyAppChrome userEmail={userEmail}>{children}</LegacyAppChrome>;
  }

  return (
    <WorkspaceShell userEmail={userEmail}>
      {children}
    </WorkspaceShell>
  );
}
