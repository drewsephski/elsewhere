"use client";

import { useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { cloudHostFetch } from "@/lib/cloud-api";
import { Spinner } from "@/components/ui/spinner";
import {
  parseBotChatReturnTo,
  type GithubOAuthCompleteResponse,
} from "@/lib/connector-need";

export default function GitHubOAuthCallbackPage() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const code = searchParams.get("code");
    const state = searchParams.get("state");
    if (!code || !state) {
      setError("Missing GitHub authorization parameters.");
      return;
    }
    const installationIdRaw = searchParams.get("installation_id");
    const installationId = installationIdRaw ? Number(installationIdRaw) : undefined;

    void (async () => {
      const response = await cloudHostFetch("/v1/connectors/github/oauth/complete", {
        method: "POST",
        body: JSON.stringify({
          code,
          state,
          ...(Number.isFinite(installationId) ? { installationId } : {}),
        }),
      });
      if (!response.ok) {
        const body = (await response.json().catch(() => null)) as { error?: string } | null;
        setError(body?.error ?? "Could not complete GitHub connection.");
        return;
      }
      const body = (await response.json()) as GithubOAuthCompleteResponse;
      router.replace(parseBotChatReturnTo(body.returnTo ?? "") ?? "/app/connectors");
      router.refresh();
    })();
  }, [router, searchParams]);

  if (error) {
    return (
      <p className="text-sm text-destructive" role="alert">
        {error}
      </p>
    );
  }

  return (
    <div className="flex items-center gap-2 text-sm text-muted-foreground">
      <Spinner className="size-4" />
      Finishing GitHub connection…
    </div>
  );
}
