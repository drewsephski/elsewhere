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

const AVATAR_PALETTE = [
  "bg-sky-100 text-sky-800",
  "bg-violet-100 text-violet-800",
  "bg-amber-100 text-amber-900",
  "bg-emerald-100 text-emerald-800",
  "bg-rose-100 text-rose-800",
  "bg-slate-200 text-slate-800",
];

export function getBotAvatarClass(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i += 1) {
    hash = (hash + name.charCodeAt(i)) % AVATAR_PALETTE.length;
  }
  return AVATAR_PALETTE[hash];
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
