import { readFileSync } from "fs";
import { describe, expect, it } from "vitest";

const daemonSource = readFileSync(new URL("./browser-daemon.mjs", import.meta.url), "utf8");

describe("browser daemon scroll action", () => {
  it("moves the mouse then wheels at the requested viewport point", () => {
    expect(daemonSource).toContain('case "scroll":');
    const scrollBlock = daemonSource.slice(daemonSource.indexOf('case "scroll":'));
    const nextCase = scrollBlock.indexOf('case "press":');
    const body = nextCase === -1 ? scrollBlock : scrollBlock.slice(0, nextCase);
    expect(body).toContain("page.mouse.move");
    expect(body).toContain("page.mouse.wheel");
    expect(body).toContain("refreshPreviewCache(page)");
  });
});
