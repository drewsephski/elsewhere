import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { cloudShellAuthenticatedRoutes } from "@/lib/cloud-shell-routes";

const cloudShellSource = readFileSync(
  resolve(import.meta.dirname, "./cloud-shell.tsx"),
  "utf8",
);

function pathPatternToRegex(pattern: string): RegExp {
  if (pattern === "index") {
    return /<Route index element=/;
  }
  const escaped = pattern
    .replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
    .replace(/:id/g, "[^/\"]+");
  return new RegExp(`path="${escaped}"|path='${escaped}'`);
}

describe("CloudShell route matrix", () => {
  it("registers every authenticated /app route from the manifest", () => {
    expect(cloudShellSource).toContain('path="/app"');
    for (const route of cloudShellAuthenticatedRoutes) {
      expect(cloudShellSource).toMatch(pathPatternToRegex(route.path));
    }
  });

  it("includes connectors and channels parity routes", () => {
    expect(cloudShellSource).toContain('path="connectors"');
    expect(cloudShellSource).toContain('path="channels"');
    expect(cloudShellSource).toContain("routines/:id");
  });
});
