import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const selectCloudHostUpstream = vi.fn();

vi.mock("@/lib/cloud-host-server", () => ({
  CloudHostUnavailableError: class CloudHostUnavailableError extends Error {
    attemptedSources: string[] = [];
  },
  selectCloudHostUpstream,
}));

describe("public Slack Events API ingress", () => {
  beforeEach(() => {
    selectCloudHostUpstream.mockReset();
    selectCloudHostUpstream.mockResolvedValue({
      baseUrl: "http://runner.internal:8080",
      source: "server",
    });
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ ok: true }), { status: 200 }),
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

  it("forwards the untouched raw body and Slack signature headers without cookies", async () => {
    const POST = await loadPost();
    const raw = '{"type":"url_verification","challenge":"abc"}';
    const response = await POST(
      new Request("http://localhost/api/channels/slack/events", {
        method: "POST",
        headers: {
          "content-type": "application/json",
          cookie: "better-auth.session=secret",
          authorization: "Bearer should-not-forward",
          "x-slack-signature": "v0=abc",
          "x-slack-request-timestamp": "123",
        },
        body: raw,
      }),
    );
    expect(response.status).toBe(200);
    expect(fetch).toHaveBeenCalledTimes(1);
    const [url, init] = (fetch as ReturnType<typeof vi.fn>).mock.calls[0] as [
      string,
      RequestInit,
    ];
    expect(url).toBe(
      "http://runner.internal:8080/internal/hooks/channels/slack/events",
    );
    const headers = init.headers as Headers;
    expect(headers.get("cookie")).toBeNull();
    expect(headers.get("authorization")).toBeNull();
    expect(headers.get("x-slack-signature")).toBe("v0=abc");
    expect(headers.get("x-slack-request-timestamp")).toBe("123");
    expect(init.body).toBeInstanceOf(Buffer);
    expect(Buffer.from(init.body as Buffer).toString("utf8")).toBe(raw);
  });
});
