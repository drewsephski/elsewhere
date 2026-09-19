// @vitest-environment happy-dom

import { afterEach, describe, expect, it } from "vitest";
import {
  readSessionStorage,
  removeSessionStorage,
  writeSessionStorage,
} from "./session-storage";

afterEach(() => {
  sessionStorage.clear();
});

describe("session storage in a browser", () => {
  it("reads, writes, and removes values", () => {
    expect(writeSessionStorage("elsewhere:test", "hello")).toBe(true);
    expect(readSessionStorage("elsewhere:test")).toBe("hello");
    removeSessionStorage("elsewhere:test");
    expect(readSessionStorage("elsewhere:test")).toBeNull();
  });

  it("returns null for missing keys", () => {
    expect(readSessionStorage("elsewhere:missing")).toBeNull();
  });
});
