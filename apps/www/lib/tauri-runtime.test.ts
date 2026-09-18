import { describe, expect, test } from "vitest";
import { isTauriRuntime } from "@/lib/tauri-runtime";

describe("isTauriRuntime", () => {
  test("returns false in Node test environment", () => {
    expect(isTauriRuntime()).toBe(false);
  });
});
