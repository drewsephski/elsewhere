import {
  cloudHostUpstreamCandidates,
  type CloudHostUpstreamCandidate,
} from "@/lib/cloud-host-upstream";

const HEALTH_CACHE_MS = 30_000;
const HEALTH_PROBE_TIMEOUT_MS = 3_000;

let cachedHealthy:
  | { candidate: CloudHostUpstreamCandidate; expiresAt: number }
  | null = null;

export class CloudHostUnavailableError extends Error {
  readonly attemptedSources: string[];

  constructor(attemptedSources: string[]) {
    super("Workspace runner is temporarily unreachable");
    this.name = "CloudHostUnavailableError";
    this.attemptedSources = attemptedSources;
  }
}

function signalWithTimeout(parent: AbortSignal | undefined, timeoutMs: number) {
  const timeoutController = new AbortController();
  const timer = setTimeout(() => timeoutController.abort(), timeoutMs);
  const signal = parent
    ? AbortSignal.any([parent, timeoutController.signal])
    : timeoutController.signal;
  return { signal, clear: () => clearTimeout(timer) };
}

function cachedCandidate(
  candidates: CloudHostUpstreamCandidate[],
  excluded: Set<string>,
): CloudHostUpstreamCandidate | null {
  if (!cachedHealthy || cachedHealthy.expiresAt <= Date.now()) {
    cachedHealthy = null;
    return null;
  }
  const match = candidates.find(
    (candidate) =>
      candidate.baseUrl === cachedHealthy?.candidate.baseUrl &&
      !excluded.has(candidate.baseUrl),
  );
  if (!match) {
    cachedHealthy = null;
    return null;
  }
  return match;
}

export function markCloudHostUpstreamHealthy(candidate: CloudHostUpstreamCandidate) {
  cachedHealthy = {
    candidate,
    expiresAt: Date.now() + HEALTH_CACHE_MS,
  };
}

export function invalidateCloudHostUpstream(baseUrl?: string) {
  if (!cachedHealthy || !baseUrl || cachedHealthy.candidate.baseUrl === baseUrl) {
    cachedHealthy = null;
  }
}

async function probeCandidate(
  candidate: CloudHostUpstreamCandidate,
  parentSignal?: AbortSignal,
): Promise<boolean> {
  const timeout = signalWithTimeout(parentSignal, HEALTH_PROBE_TIMEOUT_MS);
  try {
    const response = await fetch(`${candidate.baseUrl}/health`, {
      method: "GET",
      cache: "no-store",
      signal: timeout.signal,
    });
    // Any HTTP response proves transport reachability, but /health should be 2xx.
    return response.ok;
  } catch {
    return false;
  } finally {
    timeout.clear();
  }
}

/**
 * Resolve a reachable cloud-host before proxying an authenticated request.
 * This distinguishes web liveness from the web -> runner dependency path and
 * avoids sending non-idempotent requests to an endpoint we have not reached.
 */
export async function selectCloudHostUpstream(options?: {
  signal?: AbortSignal;
  excludeBaseUrls?: Iterable<string>;
}): Promise<CloudHostUpstreamCandidate> {
  const candidates = cloudHostUpstreamCandidates();
  const excluded = new Set(options?.excludeBaseUrls ?? []);

  const cached = cachedCandidate(candidates, excluded);
  if (cached) {
    return cached;
  }

  const attempted: string[] = [];
  for (const candidate of candidates) {
    if (excluded.has(candidate.baseUrl)) {
      continue;
    }
    attempted.push(candidate.source);
    if (await probeCandidate(candidate, options?.signal)) {
      markCloudHostUpstreamHealthy(candidate);
      return candidate;
    }
  }

  throw new CloudHostUnavailableError(attempted);
}

/** Test helper; never used by production code. */
export function resetCloudHostUpstreamCacheForTests() {
  cachedHealthy = null;
}
