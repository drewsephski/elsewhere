"use client";

import { useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { cloudHostFetch } from "@/lib/cloud-api";
import { Spinner } from "@/components/ui/spinner";

export default function McpOAuthCallbackPage() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const code = searchParams.get("code");
    const state = searchParams.get("state");
    if (!code || !state) {
      setError("Missing authorization parameters.");
      return;
    }

    void (async () => {
      const response = await cloudHostFetch("/v1/connectors/installs/oauth/complete", {
        method: "POST",
        body: JSON.stringify({ code, state }),
      });
      if (!response.ok) {
        const body = (await response.json().catch(() => null)) as { error?: string } | null;
        setError(body?.error ?? "Could not complete MCP connection.");
        return;
      }
      router.replace("/app/connectors");
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
      Finishing MCP connection…
    </div>
  );
}
