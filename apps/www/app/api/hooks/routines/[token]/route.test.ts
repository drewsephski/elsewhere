import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const selectCloudHostUpstream = vi.fn();

vi.mock("@/lib/cloud-host-server", () => ({
  CloudHostUnavailableError: class CloudHostUnavailableError extends Error {
    attemptedSources: string[] = [];
  },
  selectCloudHostUpstream,
}));

describe("public routine webhook ingress", () => {
  beforeEach(() => {
    selectCloudHostUpstream.mockReset();
    selectCloudHostUpstream.mockResolvedValue({
      baseUrl: "http://runner.internal:8080",
      source: "server",
    });
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ accepted: true }), { status: 202 }),
      ),
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  async function loadPost() {
    const mod = await import("./route");
    return mod.POST;
  }

  it("does not require a user session and forwards the token to the private runner", async () => {
    const POST = await loadPost();
    const token = "abcdefghijklmnopqrstuvwxyz012345";
    const response = await POST(
      new Request(`http://localhost/api/hooks/routines/${token}`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          cookie: "better-auth.session=secret",
          "idempotency-key": "delivery-1",
        },
        body: JSON.stringify({ deployment: "prod" }),
      }),
      { params: Promise.resolve({ token }) },
    );
    expect(response.status).toBe(202);
    expect(fetch).toHaveBeenCalledTimes(1);
    const [url, init] = (fetch as ReturnType<typeof vi.fn>).mock.calls[0] as [
      string,
      RequestInit,
    ];
    expect(url).toBe(
      `http://runner.internal:8080/internal/hooks/routines/${token}`,
    );
    expect((init.headers as Headers).get("cookie")).toBeNull();
    expect((init.headers as Headers).get("authorization")).toBeNull();
    expect((init.headers as Headers).get("idempotency-key")).toBe("delivery-1");
  });

  it("rejects non-JSON and oversized payloads without calling the runner", async () => {
    const POST = await loadPost();
    const token = "abcdefghijklmnopqrstuvwxyz012345";
    const plain = await POST(
      new Request(`http://localhost/api/hooks/routines/${token}`, {
        method: "POST",
        headers: { "content-type": "text/plain" },
        body: "hello",
      }),
      { params: Promise.resolve({ token }) },
    );
    expect(plain.status).toBe(415);
    expect(fetch).not.toHaveBeenCalled();

    const huge = "x".repeat(64 * 1024 + 8);
    const oversized = await POST(
      new Request(`http://localhost/api/hooks/routines/${token}`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "content-length": String(huge.length),
        },
        body: huge,
      }),
      { params: Promise.resolve({ token }) },
    );
    expect(oversized.status).toBe(413);
    expect(fetch).not.toHaveBeenCalled();
  });
});
