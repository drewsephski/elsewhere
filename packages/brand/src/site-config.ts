/** Public marketing copy and URLs shared by the web app and docs. */
export const siteConfig = {
  productName: "Elsewhere",
  tagline: "Run your agents elsewhere.",
  /** Hero headline split across lines (www). */
  heroLines: ["Run your agents", "elsewhere."] as const,
  cloudPitch:
    "Give every agent a computer of its own, with memory, routines, and approvals that keep working when you close the laptop.",
  contactEmail: "hello@elsewhere.dev",
  waitlistMailSubject: "Elsewhere cloud waitlist",
  links: {
    docs: "https://github.com/drewsephski/elsewhere",
    download: "https://github.com/drewsephski/elsewhere/releases",
  },
} as const;

export const marketingFeatures = [
  {
    title: "Bots that own the job",
    description:
      "Workers with roles, instructions, and tools. Delegate outcomes, not one-off prompts. Stream progress while they run on real infrastructure.",
  },
  {
    title: "A computer per worker",
    description:
      "Isolated Linux with terminal, files, and browser control. Same guest protocol on your Mac today and in managed sandboxes on the cloud path.",
  },
  {
    title: "Approvals and handback",
    description:
      "Gate sensitive actions, review what ran, and recover cleanly when you need to steer. Autonomy with an audit trail, not a black box.",
  },
  {
    title: "Routines, memory, connectors",
    description:
      "Schedule recurring work, keep history and config across sessions, and wire MCP and integrations without pasting secrets into chat.",
  },
] as const;

export const howItWorksSteps = [
  {
    step: "01",
    title: "Define a worker",
    detail:
      "Create a bot with a role, system instructions, and skills. Pair ChatGPT subscription execution or your chosen provider path from the dashboard.",
  },
  {
    step: "02",
    title: "Attach a computer",
    detail:
      "Provision an isolated environment for terminal, filesystem, and browser tasks. One machine per worker, strict tenant boundaries.",
  },
  {
    step: "03",
    title: "Delegate and recover",
    detail:
      "Send work from the workspace, approve when policy requires it, and pick up results later. Routines fire on schedule even when you are offline.",
  },
] as const;

export const productSurfaces = [
  { label: "Bots", detail: "Persistent workers" },
  { label: "Computers", detail: "Isolated VMs" },
  { label: "Approvals", detail: "Human gates" },
  { label: "Routines", detail: "Scheduled runs" },
  { label: "Connectors", detail: "MCP and integrations" },
] as const;

export const cloudRoadmapPhases = [
  {
    label: "Now",
    title: "Desktop + cloud workspace",
    detail: "macOS app and hosted Next.js workspace with Postgres-backed bots, computers, and approvals.",
  },
  {
    label: "Next",
    title: "Managed sandboxes",
    detail: "Provisioned cloud VMs with the same guest protocol, dispatcher, and runner hardening.",
  },
  {
    label: "Later",
    title: "Teams and policy",
    detail: "Shared workers, org controls, and cross-device sync without mixing tenants or secrets.",
  },
] as const;
