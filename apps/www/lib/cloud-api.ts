"use client";

import { abortAfter, mergeAbortSignals } from "@/lib/abort-utils";

/** Same-origin BFF; avoids CORS and localhost vs 127.0.0.1 cookie/port issues in the browser. */
const CLOUD_BROWSER_PREFIX = "/api/cloud";

export type CloudHostFetchInit = RequestInit & {
  /** Aborts the request after this many milliseconds (in addition to any passed `signal`). */
  timeoutMs?: number;
};

export type CloudApiErrorBody = {
  code?: string;
  error?: string;
  message?: string;
  retryable?: boolean;
  requestId?: string;
};

function cloudRequestUrl(path: string): string {
  const normalized = path.startsWith("/") ? path : `/${path}`;
  return `${CLOUD_BROWSER_PREFIX}${normalized}`;
}

function wrapNetworkError(error: unknown): Error {
  if (error instanceof TypeError) {
    return new Error(
      "Could not reach the Elsewhere workspace service. Check the runner connection and try again.",
    );
  }
  if (error instanceof Error) {
    return error;
  }
  return new Error("Could not reach the Elsewhere workspace service");
}

/** Read a structured BFF/cloud-host error without exposing deployment internals. */
export async function cloudHostErrorMessage(
  response: Response,
  fallback: string,
): Promise<string> {
  const text = await response.text().catch(() => "");
  if (text) {
    try {
      const body = JSON.parse(text) as CloudApiErrorBody;
      if (typeof body.error === "string" && body.error.trim()) {
        return body.error;
      }
      if (typeof body.message === "string" && body.message.trim()) {
        return body.message;
      }
    } catch {
      if (text.trim()) {
        return text.trim();
      }
    }
  }
  return `${fallback} (${response.status})`;
}

export async function cloudHostFetch(
  path: string,
  init: CloudHostFetchInit = {},
): Promise<Response> {
  const { timeoutMs, ...requestInit } = init;
  const headers = new Headers(requestInit.headers);
  if (requestInit.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const signals: AbortSignal[] = [];
  if (requestInit.signal) {
    signals.push(requestInit.signal);
  }
  if (timeoutMs !== undefined && timeoutMs > 0) {
    signals.push(abortAfter(timeoutMs));
  }
  const signal = signals.length > 0 ? mergeAbortSignals(signals) : undefined;

  try {
    return await fetch(cloudRequestUrl(path), {
      ...requestInit,
      headers,
      credentials: "include",
      signal,
    });
  } catch (error) {
    if (error instanceof DOMException && error.name === "TimeoutError") {
      return Promise.reject(
        new Error(
          "Connection check timed out. The hosted runner may be starting or temporarily unavailable.",
        ),
      );
    }
    throw wrapNetworkError(error);
  }
}

export interface SseStreamOptions {
  lastEventId?: string;
  signal?: AbortSignal;
  onEvent: (event: { id?: string; event: string; data: string }) => void;
}

/** fetch + session cookie SSE (EventSource cannot set headers). */
export async function cloudHostEventStream(
  path: string,
  options: SseStreamOptions,
): Promise<void> {
  const headers: Record<string, string> = {
    Accept: "text/event-stream",
  };
  if (options.lastEventId) {
    headers["Last-Event-ID"] = options.lastEventId;
  }

  let response: Response;
  try {
    response = await fetch(cloudRequestUrl(path), {
      headers,
      signal: options.signal,
      credentials: "include",
    });
  } catch (error) {
    throw wrapNetworkError(error);
  }
  if (!response.ok || !response.body) {
    throw new Error(await cloudHostErrorMessage(response, "Progress stream unavailable"));
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let currentId: string | undefined;
  let currentEvent = "message";
  let currentData: string[] = [];

  function flush() {
    if (currentData.length === 0) {
      return;
    }
    options.onEvent({
      id: currentId,
      event: currentEvent,
      data: currentData.join("\n"),
    });
    currentData = [];
    currentEvent = "message";
  }

  while (true) {
    const { done, value } = await reader.read();
    if (done) {
      flush();
      break;
    }
    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split("\n");
    buffer = lines.pop() ?? "";
    for (const line of lines) {
      if (line === "") {
        flush();
        currentId = undefined;
        continue;
      }
      if (line.startsWith(":")) {
        continue;
      }
      if (line.startsWith("id:")) {
        currentId = line.slice(3).trim();
        continue;
      }
      if (line.startsWith("event:")) {
        currentEvent = line.slice(6).trim();
        continue;
      }
      if (line.startsWith("data:")) {
        currentData.push(line.slice(5).trimStart());
      }
    }
  }
}
