import { describe, expect, it } from "vitest";
import {
  isJsonContentType,
  isPlausibleWebhookToken,
  parseWebhookJson,
  readBoundedWebhookBody,
  webhookForwardHeaders,
  ROUTINE_WEBHOOK_MAX_BYTES,
} from "./routine-webhook-ingress";

describe("routine webhook ingress helpers", () => {
  it("accepts high-entropy url-safe tokens and rejects junk", () => {
    expect(isPlausibleWebhookToken("abcdefghijklmnopqrstuvwx")).toBe(true);
    expect(isPlausibleWebhookToken("short")).toBe(false);
    expect(isPlausibleWebhookToken("has/slash/and+plus=")).toBe(false);
  });

  it("requires JSON content types", () => {
    expect(isJsonContentType("application/json")).toBe(true);
    expect(isJsonContentType("application/json; charset=utf-8")).toBe(true);
    expect(isJsonContentType("text/plain")).toBe(false);
    expect(isJsonContentType(null)).toBe(false);
  });

  it("forwards only event headers and never cookies", () => {
    const headers = webhookForwardHeaders(
      new Headers({
        "content-type": "application/json",
        "idempotency-key": "abc",
        "x-github-delivery": "deliv",
        cookie: "session=secret",
        authorization: "Bearer nope",
        "x-elsewhere-event-source": "github",
      }),
    );
    expect(headers.get("content-type")).toBe("application/json");
    expect(headers.get("idempotency-key")).toBe("abc");
    expect(headers.get("x-github-delivery")).toBe("deliv");
    expect(headers.get("x-elsewhere-event-source")).toBe("github");
    expect(headers.get("cookie")).toBeNull();
    expect(headers.get("authorization")).toBeNull();
  });

  it("rejects oversized bodies before JSON parse", async () => {
    const body = "x".repeat(ROUTINE_WEBHOOK_MAX_BYTES + 1);
    const result = await readBoundedWebhookBody(
      new Request("http://localhost/api/hooks/routines/token", {
        method: "POST",
        headers: { "content-type": "application/json", "content-length": String(body.length) },
        body,
      }),
    );
    expect(result).toEqual({ ok: false, status: 413, error: "payload too large" });
  });

  it("parses bounded JSON and rejects non-JSON", () => {
    expect(parseWebhookJson(new TextEncoder().encode('{"ok":true}'))).toEqual({
      ok: true,
      value: { ok: true },
    });
    expect(parseWebhookJson(new TextEncoder().encode("not-json")).ok).toBe(false);
  });
});
