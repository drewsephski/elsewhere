import { auth } from "@/lib/auth";
import { buildCloudBffForwardHeaders } from "@/lib/bff-forwarded-headers";
import { proxyCloudHostBffRequest } from "@/lib/cloud-host-server";

export const dynamic = "force-dynamic";

type RouteContext = { params: Promise<{ path?: string[] }> };

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

  return proxyCloudHostBffRequest({
    request,
    upstreamPath,
    search: incoming.search,
    headers,
    requestId,
    logicalRoute: incoming.pathname,
  });
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
