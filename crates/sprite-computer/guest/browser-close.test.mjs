import { readFileSync } from "fs";
import { describe, expect, it } from "vitest";

const daemonSource = readFileSync(new URL("./browser-daemon.mjs", import.meta.url), "utf8");

describe("browser daemon close action", () => {
  it("closes the current page without shutting down the persistent context", () => {
    expect(daemonSource).toContain('case "close":');
    expect(daemonSource).toContain("context.newPage()");
    const closeBlock = daemonSource.slice(daemonSource.indexOf('case "close":'));
    const nextCase = closeBlock.indexOf("default:");
    const body = closeBlock.slice(0, nextCase);
    expect(body).toContain("extra.close()");
    expect(body).not.toContain("resetContext()");
    expect(body).not.toContain("context.close()");
    expect(body).not.toContain("browser.close()");
  });
});
