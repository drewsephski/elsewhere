import { auth } from "@/lib/auth";
import { buildCloudBffForwardHeaders } from "@/lib/bff-forwarded-headers";
import {
  CloudHostUnavailableError,
  invalidateCloudHostUpstream,
  markCloudHostUpstreamHealthy,
  selectCloudHostUpstream,
} from "@/lib/cloud-host-server";
import type { CloudHostUpstreamCandidate } from "@/lib/cloud-host-upstream";

export const dynamic = "force-dynamic";

type RouteContext = { params: Promise<{ path?: string[] }> };

function unavailableResponse(requestId: string, attemptedSources: string[]) {
  return Response.json(
    {
      code: "workspace_upstream_unreachable",
      error: "Workspace runner is temporarily unreachable. Your saved ChatGPT connection has not been changed.",
      retryable: true,
      requestId,
      attempted: attemptedSources,
    },
    {
      status: 503,
      headers: {
        "Cache-Control": "no-store",
        "Retry-After": "3",
        "X-Elsewhere-Request-Id": requestId,
      },
    },
  );
}

function upstreamTarget(
  candidate: CloudHostUpstreamCandidate,
  upstreamPath: string,
  search: string,
) {
  return `${candidate.baseUrl}/${upstreamPath}${search}`;
}

async function forwardRequest(
  request: Request,
  candidate: CloudHostUpstreamCandidate,
  upstreamPath: string,
  search: string,
  headers: Headers,
  requestId: string,
): Promise<Response> {
  const hasBody = request.method !== "GET" && request.method !== "HEAD";
  const isEventStream = upstreamPath.endsWith("/events");
  const upstreamTimeoutMs = isEventStream ? 0 : 45_000;
  const upstreamSignal =
    upstreamTimeoutMs > 0
      ? AbortSignal.any([request.signal, AbortSignal.timeout(upstreamTimeoutMs)])
      : request.signal;

  const upstream = await fetch(upstreamTarget(candidate, upstreamPath, search), {
    method: request.method,
    headers,
    body: hasBody ? request.body : undefined,
    // Required when forwarding a streaming request body (Node fetch).
    duplex: hasBody ? "half" : undefined,
    signal: upstreamSignal,
    cache: "no-store",
  } as RequestInit);

  markCloudHostUpstreamHealthy(candidate);
  const responseHeaders = new Headers(upstream.headers);
  responseHeaders.delete("set-cookie");
  responseHeaders.set("X-Elsewhere-Upstream", candidate.source);
  responseHeaders.set("X-Elsewhere-Request-Id", requestId);

  return new Response(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: responseHeaders,
  });
}

async function proxyToCloudHost(request: Request, context: RouteContext): Promise<Response> {
  const requestId = crypto.randomUUID();
  const { path = [] } = await context.params;
  const session = await auth.api.getSession({ headers: request.headers });
  if (!session) {
    return new Response("Sign in to use the workspace API", { status: 401 });
  }

  let token: string;
  try {
    const issued = await auth.api.getToken({ headers: request.headers });
    if (!issued.token) {
      return new Response("Could not issue workspace credentials", { status: 401 });
    }
    token = issued.token;
  } catch {
    return new Response("Could not issue workspace credentials", { status: 401 });
  }

  const incoming = new URL(request.url);
  const upstreamPath = path.map((segment) => encodeURIComponent(segment)).join("/");
  const headers = buildCloudBffForwardHeaders(request.headers);
  headers.set("Authorization", `Bearer ${token}`);
  headers.set("X-Elsewhere-Request-Id", requestId);

  // Only replay methods without a body. Mutations are sent exactly once so a
  // transport error can never duplicate an approval, login, or other side effect.
  const mayRetryTransport = request.method === "GET" || request.method === "HEAD";
  const excluded = new Set<string>();
  const attemptedSources: string[] = [];

  while (!request.signal.aborted) {
    let candidate: CloudHostUpstreamCandidate;
    try {
      candidate = await selectCloudHostUpstream({
        signal: request.signal,
        excludeBaseUrls: excluded,
      });
    } catch (error) {
      if (error instanceof CloudHostUnavailableError) {
        attemptedSources.push(...error.attemptedSources);
      }
      console.error("[cloud-bff] no reachable workspace runner", {
        requestId,
        path: incoming.pathname,
        attemptedSources: [...new Set(attemptedSources)],
      });
      return unavailableResponse(requestId, [...new Set(attemptedSources)]);
    }

    attemptedSources.push(candidate.source);
    try {
      return await forwardRequest(
        request,
        candidate,
        upstreamPath,
        incoming.search,
        headers,
        requestId,
      );
    } catch (error) {
      invalidateCloudHostUpstream(candidate.baseUrl);
      excluded.add(candidate.baseUrl);
      console.error("[cloud-bff] workspace runner request failed", {
        requestId,
        path: incoming.pathname,
        upstream: candidate.source,
        error: error instanceof Error ? error.message : String(error),
      });
      if (!mayRetryTransport) {
        return unavailableResponse(requestId, [...new Set(attemptedSources)]);
      }
    }
  }

  return unavailableResponse(requestId, [...new Set(attemptedSources)]);
}

export function GET(request: Request, context: RouteContext) {
  return proxyToCloudHost(request, context);
}

export function POST(request: Request, context: RouteContext) {
  return proxyToCloudHost(request, context);
}

export function PUT(request: Request, context: RouteContext) {
  return proxyToCloudHost(request, context);
}

export function PATCH(request: Request, context: RouteContext) {
  return proxyToCloudHost(request, context);
}

export function DELETE(request: Request, context: RouteContext) {
  return proxyToCloudHost(request, context);
}
