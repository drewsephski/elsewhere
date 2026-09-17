/** Mirrors crates/agent-skills `SKILL_NAME_RE` / `validate_skill_name`. */
export const SKILL_NAME_PATTERN = /^[a-z0-9]+(-[a-z0-9]+)*$/;
export const SKILL_NAME_MAX_LEN = 64;

export const SKILL_NAME_RULES =
  "Frontmatter name must be a slug: lowercase letters, digits, and single hyphens (e.g. qa-scratch-skill).";

export function validateSkillName(name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed || trimmed.length > SKILL_NAME_MAX_LEN) {
    return `Skill name must be 1 to ${SKILL_NAME_MAX_LEN} characters.`;
  }
  if (!SKILL_NAME_PATTERN.test(trimmed)) {
    return SKILL_NAME_RULES;
  }
  return null;
}

/** Best-effort parse of YAML frontmatter `name` from SKILL.md text. */
export function parseSkillFrontmatterName(skillMd: string): string | null {
  const trimmed = skillMd.trimStart();
  if (!trimmed.startsWith("---")) {
    return null;
  }
  const rest = trimmed.slice(3).replace(/^[\r\n]+/, "");
  const end = rest.indexOf("\n---");
  if (end < 0) {
    return null;
  }
  const yaml = rest.slice(0, end);
  const match = yaml.match(/^\s*name:\s*(.+)\s*$/m);
  if (!match) {
    return null;
  }
  let value = match[1].trim();
  if (
    (value.startsWith('"') && value.endsWith('"')) ||
    (value.startsWith("'") && value.endsWith("'"))
  ) {
    value = value.slice(1, -1);
  }
  return value;
}

/** Strip cloud-host `validation:` / duplicated prefixes for display. */
export function formatSkillValidationError(message: string): string {
  let next = message.trim();
  while (/^validation:\s*/i.test(next)) {
    next = next.replace(/^validation:\s*/i, "").trim();
  }
  if (/skill name must use lowercase/i.test(next)) {
    return SKILL_NAME_RULES;
  }
  if (/name must match the skill slug/i.test(next)) {
    return "Frontmatter name must match this skill's slug (kebab-case).";
  }
  return next || "Check the SKILL.md frontmatter and try again.";
}
