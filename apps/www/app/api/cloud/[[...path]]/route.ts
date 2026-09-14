import { auth } from "@/lib/auth";
import { buildCloudBffForwardHeaders } from "@/lib/bff-forwarded-headers";
import { cloudHostUpstreamBaseUrl } from "@/lib/cloud-host-upstream";

export const dynamic = "force-dynamic";

type RouteContext = { params: Promise<{ path?: string[] }> };

async function proxyToCloudHost(request: Request, context: RouteContext): Promise<Response> {
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
  const target = `${cloudHostUpstreamBaseUrl()}/${upstreamPath}${incoming.search}`;

  const headers = buildCloudBffForwardHeaders(request.headers);
  headers.set("Authorization", `Bearer ${token}`);

  const hasBody = request.method !== "GET" && request.method !== "HEAD";
  const isEventStream = incoming.pathname.includes("/events");
  const upstreamTimeoutMs = isEventStream ? 0 : 45_000;
  const upstreamSignal =
    upstreamTimeoutMs > 0
      ? AbortSignal.any(
          request.signal
            ? [request.signal, AbortSignal.timeout(upstreamTimeoutMs)]
            : [AbortSignal.timeout(upstreamTimeoutMs)],
        )
      : request.signal;

  try {
    const upstream = await fetch(target, {
      method: request.method,
      headers,
      body: hasBody ? request.body : undefined,
      // Required when forwarding a streaming request body (Node fetch).
      duplex: hasBody ? "half" : undefined,
      signal: upstreamSignal,
      cache: "no-store",
    } as RequestInit);

    const responseHeaders = new Headers(upstream.headers);
    responseHeaders.delete("set-cookie");

    return new Response(upstream.body, {
      status: upstream.status,
      statusText: upstream.statusText,
      headers: responseHeaders,
    });
  } catch {
    return new Response("Workspace API is unreachable. Is cloud-host running?", { status: 503 });
  }
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
