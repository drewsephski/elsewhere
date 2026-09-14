export type CloudHostUpstreamSource =
  | "server"
  | "fallback"
  | "legacy-internal"
  | "public-build"
  | "local-default";

export type CloudHostUpstreamCandidate = {
  baseUrl: string;
  source: CloudHostUpstreamSource;
};

function normalizeBaseUrl(raw: string, source: CloudHostUpstreamSource): string {
  const trimmed = raw.trim();
  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    throw new Error(`Invalid cloud-host URL configured by ${source}`);
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw new Error(`Cloud-host URL from ${source} must use http or https`);
  }
  if (parsed.username || parsed.password) {
    throw new Error(`Cloud-host URL from ${source} must not contain credentials`);
  }
  parsed.hash = "";
  parsed.search = "";
  return parsed.toString().replace(/\/$/, "");
}

/**
 * Server-side cloud-host destinations in priority order.
 *
 * ELSEWHERE_CLOUD_HOST_URL is the canonical runtime setting for hosted/self-hosted
 * deployments. ELSEWHERE_CLOUD_HOST_FALLBACK_URL is an optional explicit HTTPS (or
 * alternate) route for migration and incident recovery. Legacy and build-time keys
 * remain for backwards compatibility only.
 */
export function cloudHostUpstreamCandidates(
  env: NodeJS.ProcessEnv = process.env,
): CloudHostUpstreamCandidate[] {
  const configured: Array<[string | undefined, CloudHostUpstreamSource]> = [
    [env.ELSEWHERE_CLOUD_HOST_URL, "server"],
    [env.ELSEWHERE_CLOUD_HOST_FALLBACK_URL, "fallback"],
    [env.ELSEWHERE_CLOUD_HOST_INTERNAL_URL, "legacy-internal"],
    [env.NEXT_PUBLIC_ELSEWHERE_CLOUD_HOST_URL, "public-build"],
  ];

  const candidates: CloudHostUpstreamCandidate[] = [];
  const seen = new Set<string>();
  for (const [raw, source] of configured) {
    if (!raw?.trim()) {
      continue;
    }
    const baseUrl = normalizeBaseUrl(raw, source);
    if (seen.has(baseUrl)) {
      continue;
    }
    seen.add(baseUrl);
    candidates.push({ baseUrl, source });
  }

  if (candidates.length === 0 && env.NODE_ENV !== "production") {
    candidates.push({
      baseUrl: "http://127.0.0.1:8080",
      source: "local-default",
    });
  }

  if (candidates.length === 0) {
    throw new Error(
      "Cloud-host upstream is not configured. Set ELSEWHERE_CLOUD_HOST_URL for this deployment.",
    );
  }

  return candidates;
}

/** First configured upstream for server-only routes that do not need failover. */
export function cloudHostUpstreamBaseUrl(): string {
  return cloudHostUpstreamCandidates()[0].baseUrl;
}
