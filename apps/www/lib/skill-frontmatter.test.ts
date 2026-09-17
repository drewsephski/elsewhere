import { describe, expect, it } from "vitest";
import {
  formatSkillValidationError,
  parseSkillFrontmatterName,
  validateSkillName,
} from "./skill-frontmatter";

describe("validateSkillName", () => {
  it("accepts kebab-case slugs", () => {
    expect(validateSkillName("qa-scratch-skill")).toBeNull();
    expect(validateSkillName("a")).toBeNull();
    expect(validateSkillName("skill1")).toBeNull();
  });

  it("rejects human-readable names", () => {
    expect(validateSkillName("QA Scratch Skill")).toMatch(/slug/i);
    expect(validateSkillName("QA-Scratch")).toMatch(/slug/i);
    expect(validateSkillName("-leading")).toMatch(/slug/i);
  });
});

describe("parseSkillFrontmatterName", () => {
  it("reads name from SKILL.md frontmatter", () => {
    const md = `---\nname: QA Scratch Skill\ndescription: test\n---\n\nbody\n`;
    expect(parseSkillFrontmatterName(md)).toBe("QA Scratch Skill");
  });
});

describe("formatSkillValidationError", () => {
  it("maps API skill name errors to the slug rule", () => {
    expect(
      formatSkillValidationError(
        "validation: skill name must use lowercase letters, digits, and single hyphens (no leading/trailing hyphen)",
      ),
    ).toMatch(/kebab-case|slug/i);
  });
});
