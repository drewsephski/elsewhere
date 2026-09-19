import { cloudHostFetch } from "@/lib/cloud-api";

export interface BrowserControlState {
  holder: "bot" | "human";
  leaseId: string | null;
  acquiredAt: string | null;
  heartbeatAt: string | null;
  youHaveControl: boolean;
}

async function readJsonError(response: Response, fallback: string): Promise<string> {
  const body = await response.text();
  if (!body) {
    return fallback;
  }
  try {
    const parsed = JSON.parse(body) as { error?: string };
    return parsed.error ?? body;
  } catch {
    return body;
  }
}

export async function fetchBrowserControlState(computerId: string): Promise<BrowserControlState> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser-control`,
  );
  if (!response.ok) {
    throw new Error(await readJsonError(response, "Could not load browser control state"));
  }
  return response.json() as Promise<BrowserControlState>;
}

export async function takeBrowserControl(computerId: string): Promise<BrowserControlState> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser-control/take`,
    { method: "POST" },
  );
  if (!response.ok) {
    throw new Error(await readJsonError(response, "Could not take browser control"));
  }
  return response.json() as Promise<BrowserControlState>;
}

export async function returnBrowserControl(computerId: string): Promise<BrowserControlState> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser-control/return`,
    { method: "POST" },
  );
  if (!response.ok) {
    throw new Error(await readJsonError(response, "Could not return browser control"));
  }
  return response.json() as Promise<BrowserControlState>;
}

export async function heartbeatBrowserControl(computerId: string): Promise<BrowserControlState> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser-control/heartbeat`,
    { method: "POST" },
  );
  if (!response.ok) {
    throw new Error(await readJsonError(response, "Could not refresh browser control lease"));
  }
  return response.json() as Promise<BrowserControlState>;
}

export async function navigateComputerBrowser(computerId: string, url: string): Promise<void> {
  const trimmed = url.trim();
  if (!trimmed) {
    throw new Error("Enter a URL to open");
  }
  const normalized =
    trimmed.startsWith("http://") || trimmed.startsWith("https://")
      ? trimmed
      : `https://${trimmed}`;
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser/navigate`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ url: normalized }),
    },
  );
  if (!response.ok) {
    throw new Error(await readJsonError(response, "Could not navigate browser"));
  }
}

export async function clickComputerBrowserPoint(
  computerId: string,
  xRatio: number,
  yRatio: number,
): Promise<void> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser/click`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ xRatio, yRatio }),
    },
  );
  if (!response.ok) {
    throw new Error(await readJsonError(response, "Could not click in browser"));
  }
}

export async function scrollComputerBrowser(
  computerId: string,
  xRatio: number,
  yRatio: number,
  deltaX: number,
  deltaY: number,
): Promise<void> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser/scroll`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ xRatio, yRatio, deltaX, deltaY }),
    },
  );
  if (!response.ok) {
    throw new Error(await readJsonError(response, "Could not scroll in browser"));
  }
}

export async function typeComputerBrowser(
  computerId: string,
  refId: string,
  text: string,
  submit = false,
): Promise<void> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser/type`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ ref: refId, text, submit }),
    },
  );
  if (!response.ok) {
    throw new Error(await readJsonError(response, "Could not type in browser"));
  }
}

export async function typeComputerBrowserFocused(
  computerId: string,
  text: string,
  submit = false,
): Promise<void> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser/type`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ text, submit }),
    },
  );
  if (!response.ok) {
    throw new Error(await readJsonError(response, "Could not type in browser"));
  }
}

export async function closeComputerBrowser(computerId: string): Promise<void> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser/close`,
    { method: "POST" },
  );
  if (!response.ok) {
    throw new Error(await readJsonError(response, "Could not close the current browser page"));
  }
}

export async function pressComputerBrowserKey(computerId: string, key: string): Promise<void> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser/press-key`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ key }),
    },
  );
  if (!response.ok) {
    throw new Error(await readJsonError(response, "Could not send key press"));
  }
}
