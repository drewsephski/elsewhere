const GENERIC_PROMPTS = [
  "What can you take on?",
  "Look around the computer",
  "Propose a first task",
] as const;

const PROMPTS_BY_ROLE = {
  researcher: [
    "Write a sourced brief",
    "Compare these options",
    "Find the latest",
  ],
  writer: [
    "Draft a first pass",
    "Tighten this copy",
    "Rewrite the tone",
  ],
  analyst: [
    "Recommend a decision",
    "Show your work",
    "Chart the tradeoffs",
  ],
  finance: [
    "Model the costs",
    "Draft a budget",
    "Flag risky spend",
  ],
  engineer: [
    "Inspect the workspace",
    "Propose a verified fix",
    "Document the setup",
  ],
  designer: [
    "Critique the live UI",
    "Propose a layout",
    "Check accessibility",
  ],
  marketing: [
    "Write a campaign brief",
    "Map audience pain",
    "Draft the launch",
  ],
  chiefOfStaff: [
    "Prep a briefing",
    "Triage this week",
    "Close open loops",
  ],
  ceo: [
    "Clarify the goal",
    "Prioritize this week",
    "Name the tradeoffs",
  ],
  projectManager: [
    "Break this into steps",
    "Surface the risks",
    "Draft a status note",
  ],
  support: [
    "Troubleshoot this",
    "Draft a reply",
    "Escalate with context",
  ],
  creativeDirector: [
    "Set the direction",
    "Critique the brand fit",
    "Name what's off",
  ],
} as const;

type BotRole = keyof typeof PROMPTS_BY_ROLE;

const ROLE_BY_AVATAR_ID: Record<string, BotRole> = {
  "amber-puff": "researcher",
  "rose-sprout": "writer",
  "indigo-kitty": "analyst",
  "slate-bunny": "finance",
  "teal-wisp": "engineer",
  "violet-kitty": "designer",
  "coral-puff": "marketing",
  "berry-puff": "chiefOfStaff",
  "sky-wisp": "ceo",
  "emerald-bunny": "projectManager",
  "lime-sprout": "support",
  "sunset-sprout": "creativeDirector",
};

const ROLE_BY_NAME: Record<string, BotRole> = {
  researcher: "researcher",
  writer: "writer",
  analyst: "analyst",
  finance: "finance",
  engineer: "engineer",
  designer: "designer",
  marketing: "marketing",
  "chief of staff": "chiefOfStaff",
  ceo: "ceo",
  "project manager": "projectManager",
  support: "support",
  "creative director": "creativeDirector",
};

export function botPresetPrompts(bot: {
  avatarId?: string | null;
  name?: string | null;
}): readonly string[] {
  const roleFromAvatar = bot.avatarId ? ROLE_BY_AVATAR_ID[bot.avatarId] : undefined;
  if (roleFromAvatar) {
    return PROMPTS_BY_ROLE[roleFromAvatar];
  }

  const roleFromName = bot.name?.trim().toLowerCase();
  if (roleFromName) {
    const role = ROLE_BY_NAME[roleFromName];
    if (role) {
      return PROMPTS_BY_ROLE[role];
    }
  }

  return GENERIC_PROMPTS;
}
