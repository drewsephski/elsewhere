export const ROUTINE_WEBHOOK_MAX_BYTES = 64 * 1024;

const FORWARDED_HEADERS = [
  "content-type",
  "idempotency-key",
  "x-github-delivery",
  "x-request-id",
  "x-elsewhere-event-source",
  "x-github-event",
] as const;

export function isPlausibleWebhookToken(token: string): boolean {
  return (
    token.length >= 16 &&
    token.length <= 128 &&
    /^[A-Za-z0-9_-]+$/.test(token)
  );
}

export function isJsonContentType(value: string | null): boolean {
  if (!value) {
    return false;
  }
  const media = value.split(";")[0]?.trim().toLowerCase();
  return media === "application/json";
}

export function webhookForwardHeaders(incoming: Headers): Headers {
  const headers = new Headers();
  for (const name of FORWARDED_HEADERS) {
    const value = incoming.get(name);
    if (value) {
      headers.set(name, value);
    }
  }
  return headers;
}

export type BoundedBodyResult =
  | { ok: true; body: Uint8Array }
  | { ok: false; status: number; error: string };

export async function readBoundedWebhookBody(
  request: Request,
  maxBytes = ROUTINE_WEBHOOK_MAX_BYTES,
): Promise<BoundedBodyResult> {
  const declared = request.headers.get("content-length");
  if (declared) {
    const length = Number(declared);
    if (Number.isFinite(length) && length > maxBytes) {
      return { ok: false, status: 413, error: "payload too large" };
    }
  }

  const reader = request.body?.getReader();
  if (!reader) {
    const buffer = await request.arrayBuffer();
    if (buffer.byteLength > maxBytes) {
      return { ok: false, status: 413, error: "payload too large" };
    }
    return { ok: true, body: new Uint8Array(buffer) };
  }

  const chunks: Uint8Array[] = [];
  let received = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }
    received += value.byteLength;
    if (received > maxBytes) {
      await reader.cancel().catch(() => undefined);
      return { ok: false, status: 413, error: "payload too large" };
    }
    chunks.push(value);
  }
  const body = new Uint8Array(received);
  let offset = 0;
  for (const chunk of chunks) {
    body.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return { ok: true, body };
}

export function parseWebhookJson(body: Uint8Array): { ok: true; value: unknown } | { ok: false } {
  try {
    const text = new TextDecoder().decode(body);
    return { ok: true, value: JSON.parse(text) };
  } catch {
    return { ok: false };
  }
}
