import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const getSession = vi.fn();
const getToken = vi.fn();
const proxyCloudHostBffRequest = vi.fn();

vi.mock("@/lib/auth", () => ({
  auth: {
    api: {
      getSession,
      getToken,
    },
  },
}));

vi.mock("@/lib/cloud-host-server", () => ({
  proxyCloudHostBffRequest,
}));

describe("cloud BFF route", () => {
  beforeEach(() => {
    getSession.mockReset();
    getToken.mockReset();
    proxyCloudHostBffRequest.mockReset();
    proxyCloudHostBffRequest.mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), { status: 200 }),
    );
  });

  afterEach(() => {
    vi.resetModules();
  });

  async function loadHandlers() {
    const mod = await import("./route");
    return mod;
  }

  it("returns 401 when there is no session", async () => {
    getSession.mockResolvedValue(null);
    const { GET } = await loadHandlers();
    const response = await GET(
      new Request("http://localhost/api/cloud/v1/bots"),
      { params: Promise.resolve({ path: ["v1", "bots"] }) },
    );
    expect(response.status).toBe(401);
    expect(proxyCloudHostBffRequest).not.toHaveBeenCalled();
  });

  it("returns 401 when token issuance fails", async () => {
    getSession.mockResolvedValue({ user: { id: "u1" } });
    getToken.mockRejectedValue(new Error("no token"));
    const { GET } = await loadHandlers();
    const response = await GET(
      new Request("http://localhost/api/cloud/v1/bots"),
      { params: Promise.resolve({ path: ["v1", "bots"] }) },
    );
    expect(response.status).toBe(401);
  });

  it("forwards bearer token, query string, and request id", async () => {
    getSession.mockResolvedValue({ user: { id: "u1" } });
    getToken.mockResolvedValue({ token: "jwt-test" });
    const { GET } = await loadHandlers();
    const request = new Request(
      "http://localhost/api/cloud/v1/bots?limit=5",
      {
        headers: {
          "x-request-id": "client-req",
          "accept-language": "en-US",
          cookie: "session=secret",
        },
      },
    );
    await GET(request, { params: Promise.resolve({ path: ["v1", "bots"] }) });

    expect(proxyCloudHostBffRequest).toHaveBeenCalledTimes(1);
    const args = proxyCloudHostBffRequest.mock.calls[0][0];
    expect(args.upstreamPath).toBe("v1/bots");
    expect(args.search).toBe("?limit=5");
    expect(args.headers.get("Authorization")).toBe("Bearer jwt-test");
    expect(args.headers.get("X-Elsewhere-Request-Id")).toBeTruthy();
    expect(args.headers.get("cookie")).toBeNull();
    expect(args.logicalRoute).toBe("/api/cloud/v1/bots");
  });
});
