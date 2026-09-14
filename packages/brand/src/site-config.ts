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
    docs: "https://github.com/drewsepeczi/elsewhere",
    download: "https://github.com/drewsepeczi/elsewhere/releases",
  },
} as const;

export const marketingFeatures = [
  {
    title: "Agent computers",
    description:
      "Each bot gets a dedicated Linux environment for terminal, files, and browser work — locally today, in the cloud tomorrow.",
  },
  {
    title: "Always-on routines",
    description:
      "Schedule skills and background jobs on infrastructure built for long-running agents, not one-shot chat sessions.",
  },
  {
    title: "Memory that travels",
    description:
      "Conversation history, bot config, and long-term memory designed to sync when you move from desktop to cloud.",
  },
  {
    title: "Approvals you control",
    description:
      "See what your agent did, approve sensitive actions, and keep a clear audit trail as autonomy increases.",
  },
] as const;

export const cloudRoadmapPhases = [
  {
    label: "Now",
    title: "Desktop + local VM",
    detail: "macOS app with SQLite and an isolated guest for agent tooling.",
  },
  {
    label: "Next",
    title: "Managed sandboxes",
    detail: "Provisioned cloud VMs with the same guest protocol and host bridge.",
  },
  {
    label: "Later",
    title: "Teams & sync",
    detail: "Shared bots, org policies, and cross-device state without sacrificing isolation.",
  },
] as const;
