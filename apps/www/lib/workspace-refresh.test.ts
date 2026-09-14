import { describe, expect, test } from "vitest";
import { WORKSPACE_ROOT } from "@/lib/computer-workspace";
import {
  filterPublicWorkspaceEntries,
  isInternalWorkspaceEntry,
  pathsToRefreshOnInvalidation,
  workspaceRevisionChanged,
} from "@/lib/workspace-refresh";

describe("workspace refresh helpers", () => {
  test("hides Elsewhere housekeeping entries", () => {
    expect(isInternalWorkspaceEntry(".elsewhere-bootstrap")).toBe(true);
    expect(isInternalWorkspaceEntry(".env")).toBe(false);
    const filtered = filterPublicWorkspaceEntries([
      { name: ".elsewhere-bootstrap", path: "/workspace/.elsewhere-bootstrap", isDir: false },
      { name: "hello_world", path: "/workspace/results/run/hello_world", isDir: false },
    ]);
    expect(filtered).toHaveLength(1);
    expect(filtered[0]?.name).toBe("hello_world");
  });

  test("detects revision changes after baseline", () => {
    expect(workspaceRevisionChanged(null, 1)).toBe(false);
    expect(workspaceRevisionChanged(1, 1)).toBe(false);
    expect(workspaceRevisionChanged(1, 2)).toBe(true);
  });

  test("refreshes workspace root and expanded paths", () => {
    const paths = pathsToRefreshOnInvalidation(
      ["/workspace/results/run-1"],
      WORKSPACE_ROOT,
    );
    expect(paths).toContain(WORKSPACE_ROOT);
    expect(paths).toContain("/workspace/results/run-1");
  });
});
