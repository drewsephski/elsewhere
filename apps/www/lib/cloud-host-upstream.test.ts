import { describe, expect, it } from "vitest";
import { cloudHostUpstreamCandidates } from "./cloud-host-upstream";

function env(values: Record<string, string | undefined>): NodeJS.ProcessEnv {
  return values as NodeJS.ProcessEnv;
}

describe("cloudHostUpstreamCandidates", () => {
  it("prefers canonical server URL, then explicit fallback, then legacy/public keys", () => {
    const candidates = cloudHostUpstreamCandidates(
      env({
        NODE_ENV: "production",
        ELSEWHERE_CLOUD_HOST_URL: "http://runner.internal:8080/",
        ELSEWHERE_CLOUD_HOST_FALLBACK_URL: "https://runner-public.example.com",
        ELSEWHERE_CLOUD_HOST_INTERNAL_URL: "http://legacy.internal:8080",
        NEXT_PUBLIC_ELSEWHERE_CLOUD_HOST_URL: "https://runner.example.com/",
      }),
    );

    expect(candidates).toEqual([
      { baseUrl: "http://runner.internal:8080", source: "server" },
      { baseUrl: "https://runner-public.example.com", source: "fallback" },
      { baseUrl: "http://legacy.internal:8080", source: "legacy-internal" },
      { baseUrl: "https://runner.example.com", source: "public-build" },
    ]);
  });

  it("deduplicates identical configured destinations", () => {
    const candidates = cloudHostUpstreamCandidates(
      env({
        NODE_ENV: "production",
        ELSEWHERE_CLOUD_HOST_URL: "http://runner.internal:8080",
        ELSEWHERE_CLOUD_HOST_INTERNAL_URL: "http://runner.internal:8080/",
      }),
    );

    expect(candidates).toEqual([
      { baseUrl: "http://runner.internal:8080", source: "server" },
    ]);
  });

  it("fails closed in production when no server upstream is configured", () => {
    expect(() => cloudHostUpstreamCandidates(env({ NODE_ENV: "production" }))).toThrow(
      /Cloud-host upstream is not configured/,
    );
  });

  it("keeps zero-config localhost development behavior", () => {
    expect(cloudHostUpstreamCandidates(env({ NODE_ENV: "development" }))).toEqual([
      { baseUrl: "http://127.0.0.1:8080", source: "local-default" },
    ]);
  });

  it("rejects unsafe or malformed upstream URLs", () => {
    expect(() =>
      cloudHostUpstreamCandidates(
        env({ NODE_ENV: "production", ELSEWHERE_CLOUD_HOST_URL: "file:///tmp/runner" }),
      ),
    ).toThrow(/http or https/);

    expect(() =>
      cloudHostUpstreamCandidates(
        env({
          NODE_ENV: "production",
          ELSEWHERE_CLOUD_HOST_URL: "https://user:secret@example.com",
        }),
      ),
    ).toThrow(/must not contain credentials/);
  });
});
