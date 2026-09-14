import { diagnoseRunnerDependency } from "@/lib/cloud-host-server";

export const dynamic = "force-dynamic";

/** Dependency health for web → cloud-host. Does not expose secrets or private addresses. */
export async function GET(request: Request) {
  const diagnostics = await diagnoseRunnerDependency({
    signal: AbortSignal.any([request.signal, AbortSignal.timeout(8_000)]),
  });

  const status = diagnostics.reachable ? 200 : 503;
  return Response.json(diagnostics, {
    status,
    headers: {
      "Cache-Control": "no-store",
    },
  });
}
