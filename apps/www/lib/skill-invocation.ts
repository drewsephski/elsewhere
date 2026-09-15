export interface SkillCatalogEntry {
  id: string;
  slug: string;
  name: string;
  status: string;
}

export interface ResolvedSkillInvocation {
  skill: SkillCatalogEntry;
  task: string;
}

export function resolveSkillSlashInvocation(
  message: string,
  skills: SkillCatalogEntry[],
): ResolvedSkillInvocation | null {
  const trimmed = message.trim();
  const match = trimmed.match(/^\/([a-zA-Z0-9_-]+)(?:\s+([\s\S]*))?$/);
  if (!match) {
    return null;
  }
  const token = match[1].toLowerCase();
  const skill = skills.find(
    (entry) =>
      entry.status === "active" &&
      (entry.slug.toLowerCase() === token || entry.name.toLowerCase() === token),
  );
  if (!skill) {
    return null;
  }
  const task = (match[2] ?? "").trim();
  return {
    skill,
    task: task || `Run the ${skill.name} skill.`,
  };
}
