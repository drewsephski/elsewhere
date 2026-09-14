import { cloudHostFetch } from "@/lib/cloud-api";

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
    const body = await response.text();
    throw new Error(body || "Could not navigate browser");
  }
}

export async function resetComputerBrowserSession(computerId: string): Promise<void> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/browser/reset`,
    { method: "POST" },
  );
  if (!response.ok) {
    const body = await response.text();
    throw new Error(body || "Could not reset browser session");
  }
}
