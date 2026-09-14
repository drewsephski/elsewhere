import type { BotCreatureKind, BotCreatureColors, BotCreatureSpec } from "@/lib/bot-visual";

export interface BotAvatarPreset {
  id: string;
  label: string;
  /** Prefilled bot name in the create dialog */
  suggestedName: string;
  /** Prefilled role / instructions in the create dialog */
  suggestedRole: string;
  kind: BotCreatureKind;
  colors: BotCreatureColors;
  expression: number;
  eyeScale: number;
  blush: boolean;
  freckle: boolean;
}

const base = {
  eye: "#1E293B",
  pupil: "#0F172A",
} as const;

export const DEFAULT_BOT_AVATAR_ID = "sky-wisp";

export const BOT_AVATAR_PRESETS: BotAvatarPreset[] = [
  {
    id: "sky-wisp",
    label: "Sky",
    suggestedName: "CEO",
    suggestedRole:
      "You are my CEO bot. Clarify goals, prioritize what matters, delegate crisply, and summarize decisions and tradeoffs. Push back when plans are vague. Ask for approval before external commitments or spend.",
    kind: "wisp",
    colors: {
      body: "#38BDF8",
      bodyDark: "#0EA5E9",
      belly: "#E0F2FE",
      cheek: "#FDA4AF",
      accent: "#0369A1",
      wing: "#BAE6FD",
      ...base,
    },
    expression: 0.5,
    eyeScale: 1,
    blush: true,
    freckle: false,
  },
  {
    id: "violet-kitty",
    label: "Violet",
    suggestedName: "Designer",
    suggestedRole:
      "You are my design bot. Improve layout, hierarchy, and UX copy. Propose concrete UI directions and keep accessibility in mind. Deliver specs or assets on your computer and explain your rationale.",
    kind: "kitty",
    colors: {
      body: "#C4B5FD",
      bodyDark: "#A78BFA",
      belly: "#EDE9FE",
      cheek: "#F9A8D4",
      accent: "#7C3AED",
      wing: "#DDD6FE",
      ...base,
    },
    expression: 0.45,
    eyeScale: 1.02,
    blush: true,
    freckle: false,
  },
  {
    id: "amber-puff",
    label: "Amber",
    suggestedName: "Researcher",
    suggestedRole:
      "You are my research bot. Gather sources, compare options, and write concise briefs with citations or links saved on your computer. Flag uncertainty and separate facts from assumptions.",
    kind: "puff",
    colors: {
      body: "#FCD34D",
      bodyDark: "#FBBF24",
      belly: "#FEF3C7",
      cheek: "#FB923C",
      accent: "#D97706",
      wing: "#FDE68A",
      ...base,
    },
    expression: 0.55,
    eyeScale: 0.98,
    blush: true,
    freckle: false,
  },
  {
    id: "emerald-bunny",
    label: "Emerald",
    suggestedName: "Project Manager",
    suggestedRole:
      "You are my project manager bot. Break work into steps, track dependencies, and keep stakeholders updated. Surface risks early and propose realistic timelines.",
    kind: "bunny",
    colors: {
      body: "#34D399",
      bodyDark: "#10B981",
      belly: "#D1FAE5",
      cheek: "#F472B6",
      accent: "#047857",
      wing: "#A7F3D0",
      ...base,
    },
    expression: 0.5,
    eyeScale: 1,
    blush: true,
    freckle: false,
  },
  {
    id: "rose-sprout",
    label: "Rose",
    suggestedName: "Writer",
    suggestedRole:
      "You are my writing bot. Draft clear, on-brand copy and refine tone for the audience. Save versions on your computer and call out open questions before publishing.",
    kind: "sprout",
    colors: {
      body: "#FDA4AF",
      bodyDark: "#FB7185",
      belly: "#FFE4E6",
      cheek: "#F472B6",
      accent: "#E11D48",
      wing: "#FECDD3",
      ...base,
    },
    expression: 0.48,
    eyeScale: 1,
    blush: true,
    freckle: true,
  },
  {
    id: "teal-wisp",
    label: "Teal",
    suggestedName: "Engineer",
    suggestedRole:
      "You are my engineering bot. Implement changes carefully, run checks when possible, and document what you changed. Ask for approval before destructive or production-impacting steps.",
    kind: "wisp",
    colors: {
      body: "#2DD4BF",
      bodyDark: "#14B8A6",
      belly: "#CCFBF1",
      cheek: "#FDA4AF",
      accent: "#0D9488",
      wing: "#99F6E4",
      ...base,
    },
    expression: 0.52,
    eyeScale: 1.04,
    blush: false,
    freckle: false,
  },
  {
    id: "coral-puff",
    label: "Coral",
    suggestedName: "Marketing",
    suggestedRole:
      "You are my marketing bot. Shape messaging, campaigns, and launch narratives. Tie ideas to audience pain points and measurable outcomes.",
    kind: "puff",
    colors: {
      body: "#FB923C",
      bodyDark: "#F97316",
      belly: "#FFEDD5",
      cheek: "#FDA4AF",
      accent: "#EA580C",
      wing: "#FED7AA",
      ...base,
    },
    expression: 0.5,
    eyeScale: 1,
    blush: true,
    freckle: false,
  },
  {
    id: "indigo-kitty",
    label: "Indigo",
    suggestedName: "Analyst",
    suggestedRole:
      "You are my analyst bot. Turn data and notes into insights, charts, and recommendations. Show your work and highlight what would change your conclusion.",
    kind: "kitty",
    colors: {
      body: "#818CF8",
      bodyDark: "#6366F1",
      belly: "#E0E7FF",
      cheek: "#C4B5FD",
      accent: "#4338CA",
      wing: "#C7D2FE",
      ...base,
    },
    expression: 0.42,
    eyeScale: 1.06,
    blush: true,
    freckle: false,
  },
  {
    id: "lime-sprout",
    label: "Lime",
    suggestedName: "Support",
    suggestedRole:
      "You are my support bot. Answer questions clearly, troubleshoot step by step, and escalate when human judgment is needed. Stay patient and precise.",
    kind: "sprout",
    colors: {
      body: "#A3E635",
      bodyDark: "#84CC16",
      belly: "#ECFCCB",
      cheek: "#FDE047",
      accent: "#65A30D",
      wing: "#D9F99D",
      ...base,
    },
    expression: 0.5,
    eyeScale: 1,
    blush: true,
    freckle: true,
  },
  {
    id: "slate-bunny",
    label: "Slate",
    suggestedName: "Finance",
    suggestedRole:
      "You are my finance bot. Model costs, budgets, and scenarios. Be conservative with assumptions and flag compliance-sensitive areas for human review.",
    kind: "bunny",
    colors: {
      body: "#94A3B8",
      bodyDark: "#64748B",
      belly: "#F1F5F9",
      cheek: "#FDA4AF",
      accent: "#475569",
      wing: "#CBD5E1",
      ...base,
    },
    expression: 0.5,
    eyeScale: 0.96,
    blush: false,
    freckle: false,
  },
  {
    id: "sunset-sprout",
    label: "Sunset",
    suggestedName: "Creative Director",
    suggestedRole:
      "You are my creative director bot. Set visual and narrative direction, critique work constructively, and keep brand consistency across deliverables.",
    kind: "sprout",
    colors: {
      body: "#F472B6",
      bodyDark: "#EC4899",
      belly: "#FCE7F3",
      cheek: "#FB923C",
      accent: "#DB2777",
      wing: "#FBCFE8",
      ...base,
    },
    expression: 0.53,
    eyeScale: 1,
    blush: true,
    freckle: false,
  },
  {
    id: "berry-puff",
    label: "Berry",
    suggestedName: "Chief of Staff",
    suggestedRole:
      "You are my chief of staff bot. Coordinate priorities, prep briefings, and follow through on open loops. Keep useful files on your computer and explain results clearly.",
    kind: "puff",
    colors: {
      body: "#E879F9",
      bodyDark: "#D946EF",
      belly: "#FAE8FF",
      cheek: "#FDA4AF",
      accent: "#A21CAF",
      wing: "#F5D0FE",
      ...base,
    },
    expression: 0.5,
    eyeScale: 1.02,
    blush: true,
    freckle: true,
  },
];

const presetById = new Map(BOT_AVATAR_PRESETS.map((preset) => [preset.id, preset]));

export function getBotAvatarPreset(avatarId: string | null | undefined): BotAvatarPreset {
  if (avatarId && presetById.has(avatarId)) {
    return presetById.get(avatarId)!;
  }
  return presetById.get(DEFAULT_BOT_AVATAR_ID)!;
}

export function botAvatarFormDefaults(avatarId: string): {
  name: string;
  instructions: string;
} {
  const preset = getBotAvatarPreset(avatarId);
  return {
    name: preset.suggestedName,
    instructions: preset.suggestedRole,
  };
}

export function botCreatureSpecFromAvatar(avatarId: string | null | undefined): BotCreatureSpec {
  const preset = getBotAvatarPreset(avatarId);
  return {
    kind: preset.kind,
    colors: preset.colors,
    expression: preset.expression,
    eyeScale: preset.eyeScale,
    blush: preset.blush,
    freckle: preset.freckle,
  };
}
