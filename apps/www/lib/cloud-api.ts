"use client";

import { fetchCloudHostJwt } from "@/lib/auth-client";
import { cloudHostBaseUrl } from "@/lib/auth.shared";

// Bind every request to the current Better Auth session. A cached bearer can outlive
// sign-out or an account switch in another tab and show the previous owner's workspace.
export async function getCloudHostJwt(signal?: AbortSignal): Promise<string> {
  return fetchCloudHostJwt(signal);
}

export async function cloudHostFetch(
  path: string,
  init: RequestInit = {},
): Promise<Response> {
  const token = await getCloudHostJwt(init.signal ?? undefined);
  const headers = new Headers(init.headers);
  headers.set("Authorization", `Bearer ${token}`);
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  return fetch(`${cloudHostBaseUrl()}${path}`, {
    ...init,
    headers,
    credentials: "omit",
  });
}

export interface SseStreamOptions {
  lastEventId?: string;
  signal?: AbortSignal;
  onEvent: (event: { id?: string; event: string; data: string }) => void;
}

/** fetch + Authorization bearer SSE (EventSource cannot set headers). */
export async function cloudHostEventStream(
  path: string,
  options: SseStreamOptions,
): Promise<void> {
  const token = await getCloudHostJwt(options.signal);
  const headers: Record<string, string> = {
    Authorization: `Bearer ${token}`,
    Accept: "text/event-stream",
  };
  if (options.lastEventId) {
    headers["Last-Event-ID"] = options.lastEventId;
  }

  const response = await fetch(`${cloudHostBaseUrl()}${path}`, {
    headers,
    signal: options.signal,
  });
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
