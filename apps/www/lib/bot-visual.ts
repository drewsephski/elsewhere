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

const AVATAR_SHELL_CLASSES = [
  "bg-violet-100 text-violet-700 ring-violet-200/80",
  "bg-sky-100 text-sky-700 ring-sky-200/80",
  "bg-amber-100 text-amber-800 ring-amber-200/80",
  "bg-emerald-100 text-emerald-800 ring-emerald-200/80",
  "bg-rose-100 text-rose-700 ring-rose-200/80",
  "bg-teal-100 text-teal-800 ring-teal-200/80",
] as const;

function hashBotId(botId: string): number {
  let hash = 0;
  for (let i = 0; i < botId.length; i++) {
    hash = (hash * 31 + botId.charCodeAt(i)) | 0;
  }
  return Math.abs(hash);
}

export function botAvatarClass(botId: string): string {
  const index = hashBotId(botId) % AVATAR_SHELL_CLASSES.length;
  return AVATAR_SHELL_CLASSES[index];
}
