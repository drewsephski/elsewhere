import {
  CloudHostUnavailableError,
  selectCloudHostUpstream,
} from "@/lib/cloud-host-server";
import { readBoundedWebhookBody } from "@/lib/routine-webhook-ingress";

export const dynamic = "force-dynamic";

const SLACK_EVENT_MAX_BYTES = 256 * 1024;

function jsonError(status: number, error: string) {
  return Response.json(
    { error },
    {
      status,
      headers: { "Cache-Control": "no-store" },
    },
  );
}

function slackForwardHeaders(incoming: Headers): Headers {
  const headers = new Headers();
  for (const name of [
    "content-type",
    "x-slack-signature",
    "x-slack-request-timestamp",
    "x-slack-retry-num",
    "x-slack-retry-reason",
  ]) {
    const value = incoming.get(name);
    if (value) {
      headers.set(name, value);
    }
  }
  return headers;
}

export async function POST(request: Request) {
  const bounded = await readBoundedWebhookBody(request, SLACK_EVENT_MAX_BYTES);
  if (!bounded.ok) {
    return jsonError(bounded.status, bounded.error);
  }

  const headers = slackForwardHeaders(request.headers);
  let candidate;
  try {
    candidate = await selectCloudHostUpstream({ signal: request.signal });
  } catch (error) {
    if (error instanceof CloudHostUnavailableError) {
      return jsonError(503, "Slack endpoint is temporarily unavailable");
    }
    throw error;
  }

  try {
    const upstream = await fetch(
      `${candidate.baseUrl}/internal/hooks/channels/slack/events`,
      {
        method: "POST",
        headers,
        body: Buffer.from(bounded.body),
        signal: AbortSignal.any([request.signal, AbortSignal.timeout(8_000)]),
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
    return jsonError(503, "Slack endpoint is temporarily unavailable");
  }
}
