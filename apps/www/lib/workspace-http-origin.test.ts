// @vitest-environment happy-dom

import { isTauriRuntime } from "@/lib/tauri-runtime";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  authHttpUrl,
  cloudBffHttpUrl,
  hostedWorkspaceOrigin,
  resolveWorkspaceHttpUrl,
} from "./workspace-http-origin";

vi.mock("@/lib/tauri-runtime", () => ({
  isTauriRuntime: vi.fn(() => false),
}));

const isTauriRuntimeMock = vi.mocked(isTauriRuntime);

describe("hostedWorkspaceOrigin", () => {
  afterEach(() => {
    isTauriRuntimeMock.mockReturnValue(false);
    vi.unstubAllEnvs();
  });

  it("returns empty string in the browser", () => {
    expect(hostedWorkspaceOrigin()).toBe("");
  });

  it("uses NEXT_PUBLIC_ELSEWHERE_WEB_ORIGIN in the desktop shell", () => {
    isTauriRuntimeMock.mockReturnValue(true);
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

  it("prefixes hosted origin in packaged desktop", () => {
    isTauriRuntimeMock.mockReturnValue(true);
    vi.stubEnv("NEXT_PUBLIC_ELSEWHERE_WEB_ORIGIN", "https://hosted.test");
    expect(cloudBffHttpUrl("/v1/health")).toBe(
      "https://hosted.test/api/cloud/v1/health",
    );
    expect(authHttpUrl("/token")).toBe("https://hosted.test/api/auth/token");
  });
});
