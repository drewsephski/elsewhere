import {
  CloudHostUnavailableError,
  selectCloudHostUpstream,
} from "@/lib/cloud-host-server";
import {
  isJsonContentType,
  isPlausiblePairingId,
  pairingForwardHeaders,
  readBoundedJsonBody,
} from "@/lib/local-mac-pairing-ingress";

export const dynamic = "force-dynamic";

type RouteContext = { params: Promise<{ pairingId: string }> };

function jsonError(status: number, error: string) {
  return Response.json(
    { error },
    {
      status,
      headers: { "Cache-Control": "no-store" },
    },
  );
}

export async function POST(request: Request, context: RouteContext) {
  const { pairingId } = await context.params;
  if (!isPlausiblePairingId(pairingId)) {
    return jsonError(404, "not found");
  }
  if (!isJsonContentType(request.headers.get("content-type"))) {
    return jsonError(415, "JSON is required");
  }
  const bounded = await readBoundedJsonBody(request);
  if (!bounded.ok) {
    return jsonError(bounded.status, bounded.error);
  }

  const headers = pairingForwardHeaders(request.headers);
  let candidate;
  try {
    candidate = await selectCloudHostUpstream({ signal: request.signal });
  } catch (error) {
    if (error instanceof CloudHostUnavailableError) {
      return jsonError(503, "Pairing endpoint is temporarily unavailable");
    }
    throw error;
  }

  try {
    const upstream = await fetch(
      `${candidate.baseUrl}/v1/local-mac/pairings/${encodeURIComponent(pairingId)}/exchange`,
      {
        method: "POST",
        headers,
        body: Buffer.from(bounded.body),
        signal: AbortSignal.any([request.signal, AbortSignal.timeout(45_000)]),
        cache: "no-store",
      },
    );
    const responseHeaders = new Headers(upstream.headers);
    responseHeaders.delete("set-cookie");
    responseHeaders.set("Cache-Control", "no-store");
    return new Response(upstream.body, {
      status: upstream.status,
      statusText: upstream.statusText,
      headers: responseHeaders,
    });
  } catch {
    return jsonError(503, "Pairing endpoint is temporarily unavailable");
  }
}
