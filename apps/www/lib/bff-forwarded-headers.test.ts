import { describe, expect, it } from "vitest";
import { buildCloudBffForwardHeaders } from "./bff-forwarded-headers";

describe("buildCloudBffForwardHeaders", () => {
  it("forwards only workspace-safe headers and drops cookies", () => {
    const incoming = new Headers({
      accept: "application/json",
      "content-type": "application/json",
      "idempotency-key": "abc",
      cookie: "better-auth.session_token=secret",
      authorization: "Bearer should-not-forward",
      referer: "https://elsewhere.example/app",
    });

    const forwarded = buildCloudBffForwardHeaders(incoming);
    expect(forwarded.get("accept")).toBe("application/json");
    expect(forwarded.get("content-type")).toBe("application/json");
    expect(forwarded.get("idempotency-key")).toBe("abc");
    expect(forwarded.get("cookie")).toBeNull();
    expect(forwarded.get("authorization")).toBeNull();
    expect(forwarded.get("referer")).toBeNull();
  });
});
