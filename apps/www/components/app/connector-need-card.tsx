"use client";

import { GitHubLogo } from "@/components/icons/github-logo";
import { NeedsYouCard } from "@/components/app/needs-you-card";
import { Button } from "@/components/ui/button";
import type { BotChatReturnTo, ConnectorNeed } from "@/lib/connector-need";
import { presentationForConnectorNeed } from "@/lib/connector-need";
import { startGithubConnectorOAuth } from "@/lib/github-oauth";
import { useState } from "react";

export function ConnectorNeedCard(props: {
  need: ConnectorNeed;
  returnTo: BotChatReturnTo;
}) {
  const copy = presentationForConnectorNeed(props.need);
  const resolved = props.need.status.phase === "resolved";
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleConnect() {
    if (busy || resolved) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const started = await startGithubConnectorOAuth({ returnTo: props.returnTo });
      window.location.href = started.authorizeUrl;
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not start GitHub connection");
      setBusy(false);
    }
  }

  return (
    <NeedsYouCard
      className="mx-auto w-full max-w-lg"
      tone={resolved ? "resolved" : "pending"}
      leading={<GitHubLogo className="size-5" />}
      title={copy.title}
      reason={copy.reason}
      continuation={copy.continuation}
      detail={
        error ? (
          <p className="text-destructive" role="alert">
            {error}
          </p>
        ) : undefined
      }
      actions={
        resolved ? undefined : (
          <Button
            type="button"
            size="sm"
            className="min-h-8 w-full sm:w-auto"
            disabled={busy}
            onClick={() => void handleConnect()}
            aria-label={copy.actionLabel}
          >
            <GitHubLogo className="size-4" />
            {busy ? "Opening GitHub…" : copy.actionLabel}
          </Button>
        )
      }
    />
  );
}
