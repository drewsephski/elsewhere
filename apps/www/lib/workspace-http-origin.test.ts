// @vitest-environment happy-dom

import { isTauriRuntime } from "@/lib/tauri-runtime";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  authHttpUrl,
  cloudBffHttpUrl,
  hostedWorkspaceOrigin,
  isHostedElsewhereWebOrigin,
  resolveWorkspaceHttpUrl,
} from "./workspace-http-origin";

vi.mock("@/lib/tauri-runtime", () => ({
  isTauriRuntime: vi.fn(() => false),
}));

const isTauriRuntimeMock = vi.mocked(isTauriRuntime);

describe("isHostedElsewhereWebOrigin", () => {
  it("recognizes production hosted origin", () => {
    expect(isHostedElsewhereWebOrigin("https://elsewhere-alpha-web.fly.dev")).toBe(
      true,
    );
  });
});

describe("hostedWorkspaceOrigin", () => {
  afterEach(() => {
    isTauriRuntimeMock.mockReturnValue(false);
    vi.unstubAllEnvs();
  });

  it("returns empty string in the browser", () => {
    expect(hostedWorkspaceOrigin()).toBe("");
  });

  it("returns empty when desktop webview is already on hosted Elsewhere", () => {
    isTauriRuntimeMock.mockReturnValue(true);
    vi.stubGlobal("window", {
      location: { origin: "https://elsewhere-alpha-web.fly.dev" },
    });
    expect(hostedWorkspaceOrigin()).toBe("");
  });

  it("uses NEXT_PUBLIC_ELSEWHERE_WEB_ORIGIN for bundled Vite dev shell only", () => {
    isTauriRuntimeMock.mockReturnValue(true);
    vi.stubGlobal("window", {
      location: { origin: "http://localhost:1420" },
    });
    vi.stubEnv("NEXT_PUBLIC_ELSEWHERE_WEB_ORIGIN", "https://app.example.com/");
    expect(hostedWorkspaceOrigin()).toBe("https://app.example.com");
  });
});

describe("resolveWorkspaceHttpUrl", () => {
  afterEach(() => {
    isTauriRuntimeMock.mockReturnValue(false);
    vi.unstubAllEnvs();
  });

  it("keeps relative paths in the browser", () => {
    expect(resolveWorkspaceHttpUrl("/api/cloud/v1/bots")).toBe("/api/cloud/v1/bots");
  });

  it("prefixes hosted origin in bundled desktop dev shell", () => {
    isTauriRuntimeMock.mockReturnValue(true);
    vi.stubGlobal("window", {
      location: { origin: "http://localhost:1420" },
    });
    vi.stubEnv("NEXT_PUBLIC_ELSEWHERE_WEB_ORIGIN", "https://hosted.test");
    expect(cloudBffHttpUrl("/v1/health")).toBe(
      "https://hosted.test/api/cloud/v1/health",
    );
    expect(authHttpUrl("/token")).toBe("https://hosted.test/api/auth/token");
  });
});
