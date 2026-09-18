import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const selectCloudHostUpstream = vi.fn();

vi.mock("@/lib/cloud-host-server", () => ({
  CloudHostUnavailableError: class CloudHostUnavailableError extends Error {
    attemptedSources: string[] = [];
  },
  selectCloudHostUpstream,
}));

describe("public local Mac pairing ingress", () => {
  beforeEach(() => {
    selectCloudHostUpstream.mockReset();
    selectCloudHostUpstream.mockResolvedValue({
      baseUrl: "http://runner.internal:8080",
      source: "server",
    });
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ pairingId: "ok" }), { status: 200 }),
      ),
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it("does not require a session and never forwards cookies or JWTs", async () => {
    const { POST } = await import("./route");
    const response = await POST(
      new Request("http://localhost/api/local-mac/pairings", {
        method: "POST",
        headers: {
          "content-type": "application/json",
          cookie: "better-auth.session=secret",
          authorization: "Bearer leaked",
        },
        body: JSON.stringify({
          deviceName: "Drew's MacBook",
          installationId: "3fa85f64-5717-4562-b3fc-2c963f66afa6",
        }),
      }),
    );
    expect(response.status).toBe(200);
    expect(fetch).toHaveBeenCalledTimes(1);
    const [url, init] = (fetch as ReturnType<typeof vi.fn>).mock.calls[0] as [
      string,
      RequestInit,
    ];
    expect(url).toBe("http://runner.internal:8080/v1/local-mac/pairings");
    expect((init.headers as Headers).get("cookie")).toBeNull();
    expect((init.headers as Headers).get("authorization")).toBeNull();
  });
});
