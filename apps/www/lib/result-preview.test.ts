import { describe, expect, it } from "vitest";
import {
  isAssignmentSummaryKind,
  isMarkdownResultFile,
} from "./result-preview";

describe("assignment summary results", () => {
  it("identifies saved assignment summaries", () => {
    expect(isAssignmentSummaryKind("summary")).toBe(true);
    expect(isAssignmentSummaryKind("file")).toBe(false);
    expect(isAssignmentSummaryKind(undefined)).toBe(false);
  });

  it("still treats assignment summaries as markdown", () => {
    expect(isMarkdownResultFile("summary.md", "summary")).toBe(true);
    expect(isMarkdownResultFile("notes.txt", "file")).toBe(false);
  });
});
