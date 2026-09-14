import {
  cloudHostUpstreamCandidates,
  type CloudHostUpstreamCandidate,
} from "./cloud-host-upstream";

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
  /** Dependency injection for deterministic tests; production uses process.env. */
  env?: NodeJS.ProcessEnv;
}): Promise<CloudHostUpstreamCandidate> {
  const candidates = cloudHostUpstreamCandidates(options?.env ?? process.env);
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

/**
 * Server-only GET/HEAD helper used by result/viewer routes that do not pass
 * through /api/cloud. Transport failures may safely try the next configured
 * upstream; HTTP responses are returned as-is and are never retried.
 */
export async function fetchCloudHostRead(
  path: string,
  init: RequestInit = {},
): Promise<Response> {
  const method = (init.method ?? "GET").toUpperCase();
  if (method !== "GET" && method !== "HEAD") {
    throw new Error("fetchCloudHostRead only supports GET/HEAD");
  }

  const excluded = new Set<string>();
  const attempted: string[] = [];
  while (!(init.signal?.aborted ?? false)) {
    let candidate: CloudHostUpstreamCandidate;
    try {
      candidate = await selectCloudHostUpstream({
        signal: init.signal ?? undefined,
        excludeBaseUrls: excluded,
      });
    } catch (error) {
      if (error instanceof CloudHostUnavailableError) {
        attempted.push(...error.attemptedSources);
      }
      throw new CloudHostUnavailableError([...new Set(attempted)]);
    }

    attempted.push(candidate.source);
    try {
      const normalizedPath = path.startsWith("/") ? path : `/${path}`;
      const response = await fetch(`${candidate.baseUrl}${normalizedPath}`, {
        ...init,
        method,
      });
      markCloudHostUpstreamHealthy(candidate);
      return response;
    } catch {
      invalidateCloudHostUpstream(candidate.baseUrl);
      excluded.add(candidate.baseUrl);
    }
  }

  throw new CloudHostUnavailableError([...new Set(attempted)]);
}

/** Test helper; never used by production code. */
export function resetCloudHostUpstreamCacheForTests() {
  cachedHealthy = null;
}
