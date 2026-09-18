/** Same-origin public ingress for unpaired Mac Companion pairing. */

export const LOCAL_MAC_PAIRING_MAX_BYTES = 8 * 1024;

export function isJsonContentType(value: string | null): boolean {
  if (!value) {
    return false;
  }
  const media = value.split(";")[0]?.trim().toLowerCase();
  return media === "application/json";
}

export function isPlausiblePairingId(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
    value.trim(),
  );
}

export function pairingForwardHeaders(incoming: Headers): Headers {
  const headers = new Headers();
  const contentType = incoming.get("content-type");
  if (contentType) {
    headers.set("content-type", contentType);
  }
  const requestId = incoming.get("x-request-id") ?? incoming.get("x-elsewhere-request-id");
  if (requestId) {
    headers.set("x-elsewhere-request-id", requestId);
  }
  return headers;
}

export type BoundedBodyResult =
  | { ok: true; body: Uint8Array }
  | { ok: false; status: number; error: string };

export async function readBoundedJsonBody(
  request: Request,
  maxBytes = LOCAL_MAC_PAIRING_MAX_BYTES,
): Promise<BoundedBodyResult> {
  const declared = request.headers.get("content-length");
  if (declared) {
    const length = Number(declared);
    if (Number.isFinite(length) && length > maxBytes) {
      return { ok: false, status: 413, error: "payload too large" };
    }
  }
  const buffer = await request.arrayBuffer();
  if (buffer.byteLength > maxBytes) {
    return { ok: false, status: 413, error: "payload too large" };
  }
  return { ok: true, body: new Uint8Array(buffer) };
}
