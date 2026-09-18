export type BotPresetPrompt = {
  label: string;
  prompt: string;
};

const GENERIC_PROMPTS: readonly BotPresetPrompt[] = [
  {
    label: "What can you take on?",
    prompt:
      "Look at your instructions and this computer, then tell me three jobs you can take on next and which one you would start with.",
  },
  {
    label: "Look around the computer",
    prompt:
      "Look around this computer workspace and summarize what's here, what's in progress, and what you would do first.",
  },
  {
    label: "Propose a first task",
    prompt:
      "Propose a first concrete task you can finish in this session, why it matters, and what you will save to the results folder.",
  },
];

const PROMPTS_BY_ROLE = {
  researcher: [
    {
      label: "Write a sourced brief",
      prompt:
        "Research this topic and write a concise sourced brief: [topic]. Use current web sources, cite them, separate facts from assumptions, and save the brief plus a source list to the results folder.",
    },
    {
      label: "Compare these options",
      prompt:
        "Compare these options and recommend one: [option A] vs [option B]. Score them on the criteria that matter, cite current sources, and make the tradeoffs explicit.",
    },
    {
      label: "Find the latest",
      prompt:
        "Find the latest developments on [topic] from the last 30 days. Summarize what changed, who is saying it, and which claims are well-sourced versus speculative.",
    },
  ],
  writer: [
    {
      label: "Draft a first pass",
      prompt:
        "Draft a first pass of [piece] for [audience]. Match a clear, on-brand voice, flag open questions before publishing, and save the draft to the results folder.",
    },
    {
      label: "Tighten this copy",
      prompt:
        "Tighten this copy. Cut fluff, sharpen the claim, and keep the voice. Paste the original below:\n\n[paste copy]",
    },
    {
      label: "Rewrite the tone",
      prompt:
        "Rewrite this for a [more confident / warmer / more technical] tone without changing the facts. Paste the original below:\n\n[paste copy]",
    },
  ],
  analyst: [
    {
      label: "Recommend a decision",
      prompt:
        "Recommend a decision on [decision]. Show the options, the evidence, what would change your conclusion, and a clear recommendation.",
    },
    {
      label: "Show your work",
      prompt:
        "Analyze [dataset or notes] and show your work: key numbers, how you got them, caveats, and what the data does not prove.",
    },
    {
      label: "Chart the tradeoffs",
      prompt:
        "Chart the tradeoffs of [options]. Build a comparison the reader can scan, then recommend one path with the risks attached.",
    },
  ],
  finance: [
    {
      label: "Model the costs",
      prompt:
        "Model the costs of [initiative] over [timeframe]. State every assumption, run a conservative case, and flag anything that needs human review.",
    },
    {
      label: "Draft a budget",
      prompt:
        "Draft a budget for [project or period]. Break it into line items, note what's known versus estimated, and save the model to the results folder.",
    },
    {
      label: "Flag risky spend",
      prompt:
        "Review this spend plan and flag risky, non-compliant, or poorly evidenced items. Be conservative and say what a human should check.",
    },
  ],
  engineer: [
    {
      label: "Inspect the workspace",
      prompt:
        "Inspect this computer workspace. Summarize the project layout, how to run it, and the first issues you would tackle.",
    },
    {
      label: "Propose a verified fix",
      prompt:
        "Investigate [bug or failing test], propose a verified fix, and run the relevant checks. Document the diff and results; ask before destructive steps.",
    },
    {
      label: "Document the setup",
      prompt:
        "Document how this workspace is set up: dependencies, run commands, env, and gotchas. Save it to the results folder so the next person can start quickly.",
    },
  ],
  designer: [
    {
      label: "Critique the live UI",
      prompt:
        "Open the live UI for [url or screen] and critique hierarchy, spacing, typography, and UX copy. Capture screenshots and propose concrete changes.",
    },
    {
      label: "Propose a layout",
      prompt:
        "Propose a layout for [screen or flow]. Cover hierarchy, responsive behavior, and accessibility. Save annotated specs or screenshots to the results folder.",
    },
    {
      label: "Check accessibility",
      prompt:
        "Audit [screen or url] for accessibility: contrast, focus order, labels, and keyboard paths. List issues by severity with a suggested fix for each.",
    },
  ],
  marketing: [
    {
      label: "Write a campaign brief",
      prompt:
        "Write a campaign brief for [launch]. Include audience, pain, promise, channels, and a measurable success metric.",
    },
    {
      label: "Map audience pain",
      prompt:
        "Map the audience pain around [product or problem]. Rank the jobs-to-be-done and the language people actually use.",
    },
    {
      label: "Draft the launch",
      prompt:
        "Draft the launch narrative for [product]. Give homepage hero, email, and social variants tied to one clear outcome.",
    },
  ],
  chiefOfStaff: [
    {
      label: "Prep a briefing",
      prompt:
        "Prep a briefing for [meeting or person] on [topic]. Cover status, decisions needed, and open loops. Save it to the results folder.",
    },
    {
      label: "Triage this week",
      prompt:
        "Triage this week: pull priorities from [notes or threads], rank what matters, and list what can wait.",
    },
    {
      label: "Close open loops",
      prompt:
        "Hunt down open loops on [project]. List owners, blockers, and the next concrete action for each.",
    },
  ],
  ceo: [
    {
      label: "Clarify the goal",
      prompt:
        "Clarify the goal of [initiative]. Push back if it's vague, name the success metric, and write a one-page decision brief.",
    },
    {
      label: "Prioritize this week",
      prompt:
        "Prioritize this week for [team or company]. What must move, what can wait, and what should be delegated.",
    },
    {
      label: "Name the tradeoffs",
      prompt:
        "Name the tradeoffs in [decision]. Make the costs of each path explicit and recommend one.",
    },
  ],
  projectManager: [
    {
      label: "Break this into steps",
      prompt:
        "Break [project] into steps with owners, dependencies, and a realistic sequence. Surface what is blocking start.",
    },
    {
      label: "Surface the risks",
      prompt:
        "Surface the risks on [project]. Likelihood, impact, early warning signs, and a mitigation for each.",
    },
    {
      label: "Draft a status note",
      prompt:
        "Draft a status note for [project] that stakeholders can scan: done, in flight, blocked, and decisions needed. Save it to the results folder.",
    },
  ],
  support: [
    {
      label: "Troubleshoot this",
      prompt:
        "Troubleshoot this issue step by step: [describe the problem]. Ask for missing details, try the likely causes, and escalate if human judgment is needed.",
    },
    {
      label: "Draft a reply",
      prompt:
        "Draft a patient, precise reply to this customer message:\n\n[paste message]",
    },
    {
      label: "Escalate with context",
      prompt:
        "Write an escalation with full context for [issue]: what happened, what we tried, and what we need a human to decide.",
    },
  ],
  creativeDirector: [
    {
      label: "Set the direction",
      prompt:
        "Set creative direction for [campaign or brand]. Name the feeling, references, and constraints so the team can execute without guessing.",
    },
    {
      label: "Critique the brand fit",
      prompt:
        "Critique this work for brand fit: [describe or paste]. What lands, what's off, and what to change first.",
    },
    {
      label: "Name what's off",
      prompt:
        "Look at [work] and name what's off — visually, narratively, or in tone — then give a sharper direction.",
    },
  ],
} as const satisfies Record<string, readonly BotPresetPrompt[]>;

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
}): readonly BotPresetPrompt[] {
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
