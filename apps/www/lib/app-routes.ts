/** In-app destinations linked from marketing surfaces. */
export const appRoutes = {
  workspace: "/app",
  signIn: "/sign-in",
  signUp: "/sign-up",
  createBot: "/app?create=1",
  computers: "/app/computers",
  routines: "/app/routines",
  approvals: "/app/approvals",
  work: "/app/work",
  results: "/app/results",
  connectors: "/app/connectors",
  channels: "/app/channels",
  skills: "/app/skills",
  settings: "/app/settings",
  pairMac: "/pair/mac",
} as const;

/** Order matches `marketingFeatures` in @elsewhere/brand. */
export const marketingFeatureAppHrefs: readonly string[] = [
  appRoutes.computers,
  appRoutes.routines,
  appRoutes.workspace,
  appRoutes.approvals,
];

export const capabilityAppHrefs: Record<string, string> = {
  Sandboxes: appRoutes.computers,
  Routines: appRoutes.routines,
  Memory: appRoutes.workspace,
  Approvals: appRoutes.approvals,
};

/** Sidebar links for legacy management pages (chat lives at `/app`). */
export const appShellNavLinks = [
  { href: appRoutes.work, label: "Work" },
  { href: appRoutes.approvals, label: "Approvals" },
  { href: appRoutes.results, label: "Results" },
  { href: appRoutes.routines, label: "Routines" },
  { href: appRoutes.computers, label: "Computers" },
  { href: appRoutes.connectors, label: "Integrations" },
  { href: appRoutes.channels, label: "Channels" },
  { href: appRoutes.skills, label: "Skills" },
] as const;
