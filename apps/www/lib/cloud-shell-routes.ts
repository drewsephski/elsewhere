/**
 * `/app/*` routes the Tauri CloudShell must register for parity with `apps/www`.
 * Used by route-matrix tests — keep in sync with `src/cloud-shell.tsx`.
 */
/** Nested `path` values under the `/app` parent route in `src/cloud-shell.tsx`. */
export const cloudShellAuthenticatedRoutes = [
  { path: "index", outlet: "workspace" },
  { path: "bots", redirect: "/app?create=1" },
  { path: "bots/:id", outlet: "workspace" },
  { path: "groups/:id", outlet: "workspace" },
  { path: "computers", page: "computers" },
  { path: "routines", page: "routines" },
  { path: "routines/:id", page: "routine-detail" },
  { path: "approvals", page: "approvals" },
  { path: "work", page: "work" },
  { path: "work/:id", page: "work-detail" },
  { path: "results", page: "results" },
  { path: "skills", page: "skills" },
  { path: "skills/:id", page: "skill-detail" },
  { path: "connectors", page: "connectors" },
  { path: "connectors/github/callback", page: "connectors-github-callback" },
  { path: "connectors/mcp/callback", page: "connectors-mcp-callback" },
  { path: "channels", page: "channels" },
  { path: "channels/slack/callback", page: "channels-slack-callback" },
  { path: "settings", redirect: "settings-dialog" },
] as const;

export const cloudShellPublicRoutes = [
  "/",
  "/sign-in",
  "/sign-up",
  "/forgot-password",
  "/reset-password",
  "/pair/mac",
] as const;
