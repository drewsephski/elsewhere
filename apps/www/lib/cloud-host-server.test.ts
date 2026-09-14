import { afterEach, describe, expect, it, vi } from "vitest";
import {
  diagnoseRunnerDependency,
  resetCloudHostUpstreamCacheForTests,
  selectCloudHostUpstream,
} from "./cloud-host-server";

function env(values: Record<string, string | undefined>): NodeJS.ProcessEnv {
  return values as NodeJS.ProcessEnv;
}

afterEach(() => {
  resetCloudHostUpstreamCacheForTests();
  vi.unstubAllGlobals();
});

describe("selectCloudHostUpstream", () => {
  it("falls through to explicit HTTPS fallback when the private route is down", async () => {
    const fetchMock = vi
      .fn()
      .mockRejectedValueOnce(new TypeError("connect refused"))
      .mockResolvedValueOnce(new Response("ok", { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    const selected = await selectCloudHostUpstream({
      env: env({
        NODE_ENV: "production",
        ELSEWHERE_CLOUD_HOST_URL: "http://runner.internal:8080",
        ELSEWHERE_CLOUD_HOST_FALLBACK_URL: "https://runner.example.com",
      }),
    });

    expect(selected).toEqual({
      baseUrl: "https://runner.example.com",
      source: "fallback",
    });
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[0][0]).toBe("http://runner.internal:8080/health");
    expect(fetchMock.mock.calls[1][0]).toBe("https://runner.example.com/health");
  });

  it("caches a recently healthy destination instead of probing on every request", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response("ok", { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    const testEnv = env({
      NODE_ENV: "production",
      ELSEWHERE_CLOUD_HOST_URL: "http://runner.internal:8080",
    });

    const first = await selectCloudHostUpstream({ env: testEnv });
    const second = await selectCloudHostUpstream({ env: testEnv });

    expect(first).toEqual(second);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("returns a typed transport failure when no runner is reachable", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("unreachable")));

    await expect(
      selectCloudHostUpstream({
        env: env({
          NODE_ENV: "production",
          ELSEWHERE_CLOUD_HOST_URL: "http://runner.internal:8080",
        }),
      }),
    ).rejects.toMatchObject({
      name: "CloudHostUnavailableError",
      attemptedSources: ["server"],
    });
  });
});

describe("diagnoseRunnerDependency", () => {
  it("reports reachable upstream and optional ready state", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response("ok", { status: 200 }))
      .mockResolvedValueOnce(new Response("ok", { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    const diagnostics = await diagnoseRunnerDependency({
      env: env({
        NODE_ENV: "production",
        ELSEWHERE_CLOUD_HOST_URL: "http://runner.internal:8080",
      }),
    });

    expect(diagnostics).toMatchObject({
      configured: true,
      reachable: true,
      ready: true,
      upstream: "server",
    });
    expect(typeof diagnostics.latencyMs).toBe("number");
  });
});
