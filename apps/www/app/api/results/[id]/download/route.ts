import { auth } from "@/lib/auth";
import { cloudHostBaseUrl } from "@/lib/auth.shared";

export const dynamic = "force-dynamic";

/** Same-origin attachment route. The session's bearer stays between the web host and runner. */
export async function GET(request: Request, { params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const headers = { "Cache-Control": "private, no-store", "X-Content-Type-Options": "nosniff" };
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id)) {
    return new Response("Result not found", { status: 404, headers });
  }
  try {
    const session = await auth.api.getSession({ headers: request.headers });
    if (!session) return new Response("Sign in to download this result", { status: 401, headers });
    const { token } = await auth.api.getToken({ headers: request.headers });
    const response = await fetch(`${cloudHostBaseUrl()}/v1/results/${id}/download`, {
      headers: { Authorization: `Bearer ${token}` },
      cache: "no-store",
      signal: AbortSignal.any([request.signal, AbortSignal.timeout(15000)]),
      redirect: "error",
    });
    if (!response.ok) return new Response(response.status === 404 ? "Result not found" : "Could not download result", { status: response.status, headers });
    return new Response(response.body, {
      headers: {
        ...headers,
        "Content-Type": "application/octet-stream",
        "Content-Disposition": response.headers.get("Content-Disposition") ?? "attachment",
      },
    });
  } catch {
    return new Response("Downloads are temporarily unavailable. Try again shortly.", { status: 503, headers });
  }
}
