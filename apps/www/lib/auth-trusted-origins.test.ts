import { describe, expect, test } from "vitest";
import { authTrustedOrigins } from "@/lib/auth.shared";

describe("authTrustedOrigins", () => {
  test("includes app, base, and default desktop dev shell origins", () => {
    const origins = authTrustedOrigins({
      appOrigin: "http://localhost:3000",
      baseURL: "http://localhost:3000",
      desktopOrigins: [],
    });
    expect(origins).toContain("http://localhost:3000");
    expect(origins).toContain("http://localhost:1420");
    expect(origins).toContain("http://127.0.0.1:1420");
  });

  test("merges custom desktop origins without duplicates", () => {
    const origins = authTrustedOrigins({
      appOrigin: "https://app.example.com",
      baseURL: "https://app.example.com",
      desktopOrigins: [
        "http://localhost:1420",
        "https://app.example.com",
        "tauri://localhost",
      ],
    });
    expect(origins).toEqual([
      "https://app.example.com",
      "http://localhost:1420",
      "tauri://localhost",
    ]);
  });
});
