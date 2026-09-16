import {
  CloudHostUnavailableError,
  selectCloudHostUpstream,
} from "@/lib/cloud-host-server";
import {
  isJsonContentType,
  isPlausibleWebhookToken,
  parseWebhookJson,
  readBoundedWebhookBody,
  webhookForwardHeaders,
} from "@/lib/routine-webhook-ingress";

export const dynamic = "force-dynamic";

type RouteContext = { params: Promise<{ token: string }> };

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
  const { token } = await context.params;
  if (!isPlausibleWebhookToken(token)) {
    return jsonError(404, "not found");
  }
  if (!isJsonContentType(request.headers.get("content-type"))) {
    return jsonError(415, "JSON is required");
  }

  const bounded = await readBoundedWebhookBody(request);
  if (!bounded.ok) {
    return jsonError(bounded.status, bounded.error);
  }
  if (!parseWebhookJson(bounded.body).ok) {
    return jsonError(400, "JSON is required");
  }

  const headers = webhookForwardHeaders(request.headers);
  let candidate;
  try {
    candidate = await selectCloudHostUpstream({ signal: request.signal });
  } catch (error) {
    if (error instanceof CloudHostUnavailableError) {
      return jsonError(503, "Webhook endpoint is temporarily unavailable");
    }
    throw error;
  }

  try {
    const upstream = await fetch(
      `${candidate.baseUrl}/internal/hooks/routines/${encodeURIComponent(token)}`,
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
    return jsonError(503, "Webhook endpoint is temporarily unavailable");
  }
}
