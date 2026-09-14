import { auth } from "@/lib/auth";
import { cloudHostUpstreamBaseUrl } from "@/lib/cloud-host-upstream";

export const dynamic = "force-dynamic";

const MAX_VIEW_BYTES = 1024 * 1024;

function isLikelyText(bytes: Uint8Array): boolean {
  if (!bytes.length) {
    return true;
  }
  let control = 0;
  for (const byte of bytes) {
    if (byte === 0) {
      return false;
    }
    if (byte < 9 || (byte > 13 && byte < 32)) {
      control += 1;
    }
  }
  return control / bytes.length < 0.02;
}

/** Same-origin viewer route. Returns UTF-8 text for chat previews; binary files are flagged. */
export async function GET(request: Request, { params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const headers = { "Cache-Control": "private, no-store", "X-Content-Type-Options": "nosniff" };
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id)) {
    return Response.json({ error: "not_found" }, { status: 404, headers });
  }
  try {
    const session = await auth.api.getSession({ headers: request.headers });
    if (!session) {
      return Response.json({ error: "unauthorized" }, { status: 401, headers });
    }
    const { token } = await auth.api.getToken({ headers: request.headers });
    const response = await fetch(`${cloudHostUpstreamBaseUrl()}/v1/results/${id}/download`, {
      headers: { Authorization: `Bearer ${token}` },
      cache: "no-store",
      signal: AbortSignal.any([request.signal, AbortSignal.timeout(15000)]),
      redirect: "error",
    });
    if (!response.ok) {
      return Response.json(
        { error: response.status === 404 ? "not_found" : "unavailable" },
        { status: response.status, headers },
      );
    }
    const buffer = await response.arrayBuffer();
    const bytes = new Uint8Array(buffer);
    if (bytes.byteLength > MAX_VIEW_BYTES) {
      return Response.json(
        { error: "too_large", size: bytes.byteLength },
        { status: 413, headers },
      );
    }
    if (!isLikelyText(bytes)) {
      return Response.json({ isBinary: true, size: bytes.byteLength }, { headers });
    }
    const text = new TextDecoder("utf-8").decode(bytes);
    return Response.json({ isBinary: false, text, size: bytes.byteLength }, { headers });
  } catch {
    return Response.json({ error: "unavailable" }, { status: 503, headers });
  }
}
