import { cloudHostFetch } from "@/lib/cloud-api";
import type { BotChatReturnTo, GithubOAuthStartResponse } from "@/lib/connector-need";

export async function startGithubConnectorOAuth(options?: {
  returnTo?: BotChatReturnTo;
}): Promise<GithubOAuthStartResponse> {
  const response = await cloudHostFetch("/v1/connectors/github/oauth/start", {
    method: "POST",
    body: JSON.stringify(options?.returnTo ? { returnTo: options.returnTo } : {}),
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error ?? "GitHub App is not available. Check host configuration.");
  }
  return (await response.json()) as GithubOAuthStartResponse;
}
