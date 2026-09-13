export function getBotInitials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) {
    return "?";
  }
  if (parts.length === 1) {
    return parts[0].slice(0, 2).toUpperCase();
  }
  return `${parts[0][0] ?? ""}${parts[1][0] ?? ""}`.toUpperCase();
}

export type BotCreatureKind = "puff" | "bunny" | "kitty" | "sprout" | "wisp";

export interface BotCreatureColors {
  body: string;
  bodyDark: string;
  belly: string;
  cheek: string;
  eye: string;
  pupil: string;
  accent: string;
  wing: string;
}

export interface BotCreatureSpec {
  kind: BotCreatureKind;
  colors: BotCreatureColors;
  /** 0–1, shifts eye vertical position slightly */
  expression: number;
  /** Wider eyes for curious bots */
  eyeScale: number;
  blush: boolean;
  freckle: boolean;
}

const CREATURE_SHELL_CLASSES = [
  "bg-sky-100 ring-sky-200/80",
  "bg-violet-100 ring-violet-200/80",
  "bg-amber-100 ring-amber-200/80",
  "bg-emerald-100 ring-emerald-200/80",
  "bg-rose-100 ring-rose-200/80",
  "bg-teal-100 ring-teal-200/80",
] as const;

const CREATURE_COLOR_SETS: BotCreatureColors[] = [
  {
    body: "#7DD3FC",
    bodyDark: "#38BDF8",
    belly: "#E0F2FE",
    cheek: "#FDA4AF",
    eye: "#1E293B",
    pupil: "#0F172A",
    accent: "#0284C7",
    wing: "#BAE6FD",
  },
  {
    body: "#C4B5FD",
    bodyDark: "#A78BFA",
    belly: "#EDE9FE",
    cheek: "#F9A8D4",
    eye: "#1E293B",
    pupil: "#0F172A",
    accent: "#7C3AED",
    wing: "#DDD6FE",
  },
  {
    body: "#FCD34D",
    bodyDark: "#FBBF24",
    belly: "#FEF3C7",
    cheek: "#FB923C",
    eye: "#1E293B",
    pupil: "#0F172A",
    accent: "#D97706",
    wing: "#FDE68A",
  },
  {
    body: "#6EE7B7",
    bodyDark: "#34D399",
    belly: "#D1FAE5",
    cheek: "#F472B6",
    eye: "#1E293B",
    pupil: "#0F172A",
    accent: "#059669",
    wing: "#A7F3D0",
  },
  {
    body: "#FDA4AF",
    bodyDark: "#FB7185",
    belly: "#FFE4E6",
    cheek: "#F472B6",
    eye: "#1E293B",
    pupil: "#0F172A",
    accent: "#E11D48",
    wing: "#FECDD3",
  },
  {
    body: "#5EEAD4",
    bodyDark: "#2DD4BF",
    belly: "#CCFBF1",
    cheek: "#FDA4AF",
    eye: "#1E293B",
    pupil: "#0F172A",
    accent: "#0D9488",
    wing: "#99F6E4",
  },
];

const NAMED_CREATURE: Record<
  string,
  { kind: BotCreatureKind; colors: BotCreatureColors; shell: string }
> = {
  Scout: {
    kind: "wisp",
    shell: "bg-sky-100 ring-sky-300/70 shadow-sm shadow-sky-200/50",
    colors: {
      body: "#38BDF8",
      bodyDark: "#0EA5E9",
      belly: "#E0F2FE",
      cheek: "#FDA4AF",
      eye: "#1E293B",
      pupil: "#0F172A",
      accent: "#0369A1",
      wing: "#BAE6FD",
    },
  },
  Sky: {
    kind: "puff",
    shell: "bg-sky-50 ring-sky-200/90",
    colors: {
      body: "#7DD3FC",
      bodyDark: "#38BDF8",
      belly: "#F0F9FF",
      cheek: "#F9A8D4",
      eye: "#1E293B",
      pupil: "#0F172A",
      accent: "#0284C7",
      wing: "#E0F2FE",
    },
  },
  Craft: {
    kind: "sprout",
    shell: "bg-orange-100 ring-orange-200/80",
    colors: {
      body: "#FB923C",
      bodyDark: "#F97316",
      belly: "#FFEDD5",
      cheek: "#FDA4AF",
      eye: "#1E293B",
      pupil: "#0F172A",
      accent: "#16A34A",
      wing: "#FED7AA",
    },
  },
  Atlas: {
    kind: "bunny",
    shell: "bg-emerald-100 ring-emerald-200/80",
    colors: {
      body: "#34D399",
      bodyDark: "#10B981",
      belly: "#D1FAE5",
      cheek: "#F472B6",
      eye: "#1E293B",
      pupil: "#0F172A",
      accent: "#047857",
      wing: "#A7F3D0",
    },
  },
};

const NAMED_AVATAR_CLASSES: Record<string, string> = {
  Scout: "bg-sky-500 text-white shadow-sm shadow-sky-500/20",
  Sky: "bg-sky-400 text-white",
  Craft: "bg-orange-500 text-white",
  Atlas: "bg-emerald-600 text-white",
};

const CREATURE_KINDS: BotCreatureKind[] = [
  "puff",
  "bunny",
  "kitty",
  "sprout",
  "wisp",
];

function hashBotName(name: string): number {
  let hash = 2166136261;
  for (let i = 0; i < name.length; i += 1) {
    hash ^= name.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function pickIndex(hash: number, salt: number, length: number): number {
  return (Math.imul(hash, salt + 1) >>> 0) % length;
}

export function getBotCreatureSpec(name: string): BotCreatureSpec {
  const trimmed = name.trim();
  const named = NAMED_CREATURE[trimmed];
  if (named) {
    return {
      kind: named.kind,
      colors: named.colors,
      expression: 0.5,
      eyeScale: 1,
      blush: true,
      freckle: named.kind === "sprout",
    };
  }

  const hash = hashBotName(trimmed || "?");
  const kind = CREATURE_KINDS[pickIndex(hash, 3, CREATURE_KINDS.length)];
  const colors = CREATURE_COLOR_SETS[pickIndex(hash, 7, CREATURE_COLOR_SETS.length)];

  return {
    kind,
    colors,
    expression: (pickIndex(hash, 11, 100) + 1) / 100,
    eyeScale: 0.92 + (pickIndex(hash, 13, 18) / 100),
    blush: pickIndex(hash, 17, 10) > 1,
    freckle: pickIndex(hash, 19, 4) === 0,
  };
}

export function getBotCreatureShellClass(name: string): string {
  const trimmed = name.trim();
  const named = NAMED_CREATURE[trimmed];
  if (named) {
    return named.shell;
  }
  const hash = hashBotName(trimmed || "?");
  return CREATURE_SHELL_CLASSES[pickIndex(hash, 5, CREATURE_SHELL_CLASSES.length)];
}

export function getBotAvatarClass(name: string): string {
  const named = NAMED_AVATAR_CLASSES[name.trim()];
  if (named) {
    return named;
  }
  return getBotCreatureShellClass(name);
}

export type BotActivityIndicator = "streaming" | "active" | "recent" | null;

export function getBotActivityIndicator(
  bot: { updatedAt: number },
  selected: boolean,
  streaming: boolean,
): BotActivityIndicator {
  if (selected && streaming) {
    return "streaming";
  }
  if (selected) {
    return "active";
  }
  const dayMs = 86_400_000;
  if (Date.now() - bot.updatedAt < dayMs) {
    return "recent";
  }
  return null;
}

export function getBotSubtitle(bot: {
  description: string | null;
  systemPrompt: string;
  model: string;
}): string {
  if (bot.description?.trim()) {
    const text = bot.description.trim();
    return text.length > 48 ? `${text.slice(0, 48)}…` : text;
  }
  if (bot.systemPrompt.trim()) {
    const line = bot.systemPrompt.trim().split(/\n/)[0] ?? "";
    const snippet = line.length > 48 ? `${line.slice(0, 48)}…` : line;
    return snippet || bot.model;
  }
  return bot.model;
}

export function getBotPreview(bot: {
  description: string | null;
  systemPrompt: string;
  model: string;
}): string {
  if (bot.description?.trim()) {
    return bot.description.trim();
  }
  if (bot.systemPrompt.trim()) {
    const snippet = bot.systemPrompt.trim();
    return snippet.length > 72 ? `${snippet.slice(0, 72)}…` : snippet;
  }
  return bot.model;
}
