"use client";

/** Same-origin BFF; avoids CORS and localhost vs 127.0.0.1 cookie/port issues in the browser. */
const CLOUD_BROWSER_PREFIX = "/api/cloud";

function cloudRequestUrl(path: string): string {
  const normalized = path.startsWith("/") ? path : `/${path}`;
  return `${CLOUD_BROWSER_PREFIX}${normalized}`;
}

function wrapNetworkError(error: unknown): Error {
  if (error instanceof TypeError) {
    return new Error(
      "Workspace API is unreachable. Start cloud-host (cargo run -p cloud-host) and refresh.",
    );
  }
  if (error instanceof Error) {
    return error;
  }
  return new Error("Workspace API is unreachable");
}

export async function cloudHostFetch(
  path: string,
  init: RequestInit = {},
): Promise<Response> {
  const headers = new Headers(init.headers);
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  try {
    return await fetch(cloudRequestUrl(path), {
      ...init,
      headers,
      credentials: "include",
    });
  } catch (error) {
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
    throw new Error(`SSE request failed: ${response.status}`);
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
