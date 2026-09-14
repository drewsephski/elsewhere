import { auth } from "@/lib/auth";
import { cloudHostBaseUrl } from "@/lib/auth.shared";

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
  const target = `${cloudHostBaseUrl()}/${upstreamPath}${incoming.search}`;

  const headers = new Headers(request.headers);
  headers.set("Authorization", `Bearer ${token}`);
  headers.delete("host");
  headers.delete("connection");

  const hasBody = request.method !== "GET" && request.method !== "HEAD";

  try {
    const upstream = await fetch(target, {
      method: request.method,
      headers,
      body: hasBody ? request.body : undefined,
      // Required when forwarding a streaming request body (Node fetch).
      duplex: hasBody ? "half" : undefined,
      signal: request.signal,
      cache: "no-store",
    } as RequestInit);

    return new Response(upstream.body, {
      status: upstream.status,
      statusText: upstream.statusText,
      headers: upstream.headers,
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
