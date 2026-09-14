const ALLOWED_INCOMING = new Set([
  "accept",
  "accept-language",
  "content-type",
  "content-length",
  "idempotency-key",
  "last-event-id",
  "if-none-match",
  "cache-control",
]);

/** Strip browser/session headers; cloud-host receives only workspace API headers plus a fresh JWT. */
export function buildCloudBffForwardHeaders(incoming: Headers): Headers {
  const headers = new Headers();
  for (const [name, value] of incoming.entries()) {
    const lower = name.toLowerCase();
    if (ALLOWED_INCOMING.has(lower)) {
      headers.set(name, value);
    }
  }
  return headers;
}
