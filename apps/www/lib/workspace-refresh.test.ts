import { describe, expect, test } from "vitest";
import { WORKSPACE_ROOT } from "@/lib/computer-workspace";
import {
  filterPublicWorkspaceEntries,
  isHiddenWorkspaceListingName,
  isInternalWorkspaceEntry,
  isUserVisibleWorkspaceEntry,
  pathsToRefreshOnInvalidation,
  workspaceRevisionChanged,
} from "@/lib/workspace-refresh";

describe("workspace refresh helpers", () => {
  test("hides Elsewhere housekeeping entries", () => {
    expect(isInternalWorkspaceEntry(".elsewhere")).toBe(true);
    expect(isInternalWorkspaceEntry(".elsewhere-bootstrap")).toBe(true);
    expect(isInternalWorkspaceEntry(".env")).toBe(false);
    const filtered = filterPublicWorkspaceEntries([
      { name: ".elsewhere-bootstrap", path: "/workspace/.elsewhere-bootstrap", isDir: false },
      { name: "hello_world", path: "/workspace/results/run/hello_world", isDir: false },
    ]);
    expect(filtered).toHaveLength(1);
    expect(filtered[0]?.name).toBe("hello_world");
  });

  test("hides .elsewhere and its descendants", () => {
    expect(isUserVisibleWorkspaceEntry("/workspace/.elsewhere", ".elsewhere", true)).toBe(false);
    expect(
      isUserVisibleWorkspaceEntry("/workspace/.elsewhere/config.json", "config.json", false),
    ).toBe(false);
    const filtered = filterPublicWorkspaceEntries([
      { name: ".elsewhere", path: "/workspace/.elsewhere", isDir: true },
      { name: "config.json", path: "/workspace/.elsewhere/config.json", isDir: false },
      { name: "README.md", path: "/workspace/README.md", isDir: false },
    ]);
    expect(filtered.map((entry) => entry.name)).toEqual(["README.md"]);
  });

  test("keeps ordinary useful dotfiles and project files", () => {
    const filtered = filterPublicWorkspaceEntries([
      { name: ".env.example", path: "/workspace/.env.example", isDir: false },
      { name: ".github", path: "/workspace/.github", isDir: true },
      { name: ".gitignore", path: "/workspace/.gitignore", isDir: false },
      { name: ".cursor", path: "/workspace/.cursor", isDir: true },
      { name: ".env", path: "/workspace/.env", isDir: false },
      { name: "README.md", path: "/workspace/README.md", isDir: false },
      { name: "package.json", path: "/workspace/package.json", isDir: false },
      { name: "Cargo.toml", path: "/workspace/Cargo.toml", isDir: false },
    ]);
    expect(filtered).toHaveLength(8);
  });

  test("hides generated-noise directories and OS junk exactly as intended", () => {
    expect(isHiddenWorkspaceListingName("node_modules", true)).toBe(true);
    expect(isHiddenWorkspaceListingName("node_modules", false)).toBe(false);
    expect(isHiddenWorkspaceListingName("dist", true)).toBe(true);
    expect(isHiddenWorkspaceListingName("dist.ts", false)).toBe(false);
    expect(isUserVisibleWorkspaceEntry("/workspace/build", "build", false)).toBe(true);
    expect(isUserVisibleWorkspaceEntry("/workspace/build", "build", true)).toBe(false);
    expect(
      isUserVisibleWorkspaceEntry("/workspace/node_modules/left-pad/index.js", "index.js", false),
    ).toBe(false);

    const filtered = filterPublicWorkspaceEntries([
      { name: ".git", path: "/workspace/.git", isDir: true },
      { name: ".DS_Store", path: "/workspace/.DS_Store", isDir: false },
      { name: "node_modules", path: "/workspace/node_modules", isDir: true },
      { name: "index.js", path: "/workspace/node_modules/index.js", isDir: false },
      { name: ".next", path: "/workspace/.next", isDir: true },
      { name: "dist", path: "/workspace/dist", isDir: true },
      { name: "src", path: "/workspace/src", isDir: true },
    ]);
    expect(filtered.map((entry) => entry.name)).toEqual(["src"]);
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
