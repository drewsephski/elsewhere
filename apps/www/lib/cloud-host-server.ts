import {
  cloudHostUpstreamCandidates,
  type CloudHostUpstreamCandidate,
} from "./cloud-host-upstream";

const HEALTH_CACHE_MS = 30_000;
const HEALTH_PROBE_TIMEOUT_MS = 3_000;
const READY_PROBE_TIMEOUT_MS = 4_000;
const DEFAULT_API_TIMEOUT_MS = 45_000;
const TRANSPORT_RETRY_JITTER_MS = 250;

let cachedHealthy:
  | { candidate: CloudHostUpstreamCandidate; expiresAt: number }
  | null = null;

export type CloudHostTransportCategory =
  | "dns"
  | "tcp"
  | "timeout"
  | "abort"
  | "http"
  | "unknown";

export class CloudHostUnavailableError extends Error {
  readonly attemptedSources: string[];

  constructor(attemptedSources: string[]) {
    super("Workspace runner is temporarily unreachable");
    this.name = "CloudHostUnavailableError";
    this.attemptedSources = attemptedSources;
  }
}

export type RunnerDependencyDiagnostics = {
  configured: boolean;
  reachable: boolean;
  ready: boolean | null;
  upstream: CloudHostUpstreamCandidate["source"] | null;
  latencyMs: number | null;
  attempted: string[];
  transportError?: CloudHostTransportCategory;
};

function signalWithTimeout(parent: AbortSignal | undefined, timeoutMs: number) {
  const timeoutController = new AbortController();
  const timer = setTimeout(() => timeoutController.abort(), timeoutMs);
  const signal = parent
    ? AbortSignal.any([parent, timeoutController.signal])
    : timeoutController.signal;
  return { signal, clear: () => clearTimeout(timer) };
}

export function classifyCloudHostTransportError(error: unknown): CloudHostTransportCategory {
  if (error instanceof DOMException && error.name === "AbortError") {
    return "abort";
  }
  if (error instanceof DOMException && error.name === "TimeoutError") {
    return "timeout";
  }
  const message = error instanceof Error ? error.message.toLowerCase() : String(error).toLowerCase();
  if (message.includes("getaddrinfo") || message.includes("enotfound") || message.includes("dns")) {
    return "dns";
  }
  if (
    message.includes("econnrefused") ||
    message.includes("connect refused") ||
    message.includes("connection refused")
  ) {
    return "tcp";
  }
  if (message.includes("timeout") || message.includes("timed out")) {
    return "timeout";
  }
  return "unknown";
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

async function probeCandidateHealth(
  candidate: CloudHostUpstreamCandidate,
  parentSignal?: AbortSignal,
): Promise<{ ok: boolean; latencyMs: number; category?: CloudHostTransportCategory }> {
  const started = Date.now();
  const timeout = signalWithTimeout(parentSignal, HEALTH_PROBE_TIMEOUT_MS);
  try {
    const response = await fetch(`${candidate.baseUrl}/health`, {
      method: "GET",
      cache: "no-store",
      signal: timeout.signal,
    });
    return { ok: response.ok, latencyMs: Date.now() - started };
  } catch (error) {
    return {
      ok: false,
      latencyMs: Date.now() - started,
      category: classifyCloudHostTransportError(error),
    };
  } finally {
    timeout.clear();
  }
}

async function probeCandidateReady(
  candidate: CloudHostUpstreamCandidate,
  parentSignal?: AbortSignal,
): Promise<boolean> {
  const timeout = signalWithTimeout(parentSignal, READY_PROBE_TIMEOUT_MS);
  try {
    const response = await fetch(`${candidate.baseUrl}/ready`, {
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

function logBffEvent(
  level: "info" | "error",
  event: string,
  fields: Record<string, unknown>,
) {
  const payload = { event, ...fields };
  if (level === "error") {
    console.error("[cloud-bff]", payload);
  } else {
    console.info("[cloud-bff]", payload);
  }
}

export async function diagnoseRunnerDependency(options?: {
  signal?: AbortSignal;
  env?: NodeJS.ProcessEnv;
}): Promise<RunnerDependencyDiagnostics> {
  let candidates: CloudHostUpstreamCandidate[];
  try {
    candidates = cloudHostUpstreamCandidates(options?.env ?? process.env);
  } catch {
    return {
      configured: false,
      reachable: false,
      ready: null,
      upstream: null,
      latencyMs: null,
      attempted: [],
    };
  }

  const attempted: string[] = [];
  for (const candidate of candidates) {
    attempted.push(candidate.source);
    const health = await probeCandidateHealth(candidate, options?.signal);
    if (!health.ok) {
      continue;
    }
    markCloudHostUpstreamHealthy(candidate);
    const ready = await probeCandidateReady(candidate, options?.signal);
    return {
      configured: true,
      reachable: true,
      ready,
      upstream: candidate.source,
      latencyMs: health.latencyMs,
      attempted,
      transportError: health.category,
    };
  }

  return {
    configured: true,
    reachable: false,
    ready: null,
    upstream: null,
    latencyMs: null,
    attempted,
  };
}

export async function selectCloudHostUpstream(options?: {
  signal?: AbortSignal;
  excludeBaseUrls?: Iterable<string>;
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
    const probeStarted = Date.now();
    const health = await probeCandidateHealth(candidate, options?.signal);
    if (health.ok) {
      markCloudHostUpstreamHealthy(candidate);
      logBffEvent("info", "upstream_selected", {
        upstream: candidate.source,
        probeMs: Date.now() - probeStarted,
      });
      return candidate;
    }
    logBffEvent("error", "upstream_probe_failed", {
      upstream: candidate.source,
      probeMs: Date.now() - probeStarted,
      transportError: health.category ?? "unknown",
    });
  }

  throw new CloudHostUnavailableError(attempted);
}

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

function upstreamTarget(
  candidate: CloudHostUpstreamCandidate,
  upstreamPath: string,
  search: string,
) {
  return `${candidate.baseUrl}/${upstreamPath}${search}`;
}

function unavailableResponse(requestId: string, attemptedSources: string[]) {
  return Response.json(
    {
      code: "workspace_upstream_unreachable",
      error:
        "Workspace runner is temporarily unreachable. Your saved ChatGPT connection has not been changed.",
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

export type CloudHostBffProxyOptions = {
  request: Request;
  upstreamPath: string;
  search: string;
  headers: Headers;
  requestId: string;
  logicalRoute: string;
};

async function forwardCloudHostRequest(
  options: CloudHostBffProxyOptions,
  candidate: CloudHostUpstreamCandidate,
): Promise<Response> {
  const { request, upstreamPath, search, headers, requestId } = options;
  const hasBody = request.method !== "GET" && request.method !== "HEAD";
  const isEventStream = upstreamPath.endsWith("/events");
  const upstreamTimeoutMs = isEventStream ? 0 : DEFAULT_API_TIMEOUT_MS;
  const upstreamSignal =
    upstreamTimeoutMs > 0
      ? AbortSignal.any([request.signal, AbortSignal.timeout(upstreamTimeoutMs)])
      : request.signal;

  const upstreamStarted = Date.now();
  const upstream = await fetch(upstreamTarget(candidate, upstreamPath, search), {
    method: request.method,
    headers,
    body: hasBody ? request.body : undefined,
    duplex: hasBody ? "half" : undefined,
    signal: upstreamSignal,
    cache: "no-store",
  } as RequestInit);

  markCloudHostUpstreamHealthy(candidate);
  logBffEvent("info", "upstream_response", {
    requestId,
    method: request.method,
    route: options.logicalRoute,
    upstream: candidate.source,
    upstreamMs: Date.now() - upstreamStarted,
    status: upstream.status,
  });

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

/** Authenticated same-origin BFF proxy to cloud-host with discovery, failover, and logging. */
export async function proxyCloudHostBffRequest(
  options: CloudHostBffProxyOptions,
): Promise<Response> {
  const { request, requestId, logicalRoute } = options;
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
      logBffEvent("error", "no_reachable_upstream", {
        requestId,
        method: request.method,
        route: logicalRoute,
        attempted: [...new Set(attemptedSources)],
      });
      return unavailableResponse(requestId, [...new Set(attemptedSources)]);
    }

    attemptedSources.push(candidate.source);
    try {
      return await forwardCloudHostRequest(options, candidate);
    } catch (error) {
      invalidateCloudHostUpstream(candidate.baseUrl);
      excluded.add(candidate.baseUrl);
      logBffEvent("error", "upstream_request_failed", {
        requestId,
        method: request.method,
        route: logicalRoute,
        upstream: candidate.source,
        transportError: classifyCloudHostTransportError(error),
        failoverAttempted: mayRetryTransport && excluded.size < cloudHostUpstreamCandidates().length,
      });
      if (!mayRetryTransport) {
        return unavailableResponse(requestId, [...new Set(attemptedSources)]);
      }
      await new Promise((resolve) =>
        setTimeout(resolve, TRANSPORT_RETRY_JITTER_MS + Math.floor(Math.random() * TRANSPORT_RETRY_JITTER_MS)),
      );
    }
  }

  return unavailableResponse(requestId, [...new Set(attemptedSources)]);
}

export function resetCloudHostUpstreamCacheForTests() {
  cachedHealthy = null;
}
