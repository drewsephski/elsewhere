"use client";

import { useCallback, useEffect, useState } from "react";
import { cloudHostFetch } from "@/lib/cloud-api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

interface ConnectorSummary {
  provider: string;
  status: string;
  metadata: {
    login?: string;
    name?: string;
    avatarUrl?: string;
  };
  connectedAt?: string | null;
  updatedAt: string;
}

export function ConnectorsManager() {
  const [github, setGithub] = useState<ConnectorSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    const response = await cloudHostFetch("/v1/connectors/github");
    if (!response.ok) {
      setError("Could not load connector status");
      return;
    }
    setGithub((await response.json()) as ConnectorSummary);
    setError(null);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleConnect() {
    setBusy(true);
    setError(null);
    try {
      const response = await cloudHostFetch("/v1/connectors/github/oauth/start", {
        method: "POST",
      });
      if (!response.ok) {
        setError("GitHub OAuth is not available. Check host configuration.");
        return;
      }
      const body = (await response.json()) as { authorizeUrl: string };
      window.location.href = body.authorizeUrl;
    } finally {
      setBusy(false);
    }
  }

  async function handleDisconnect() {
    setBusy(true);
    setError(null);
    try {
      const response = await cloudHostFetch("/v1/connectors/github", {
        method: "DELETE",
      });
      if (!response.ok) {
        setError("Could not disconnect GitHub");
        return;
      }
      await load();
    } finally {
      setBusy(false);
    }
  }

  const connected = github?.status === "connected";

  return (
    <div className="space-y-4">
      {error ? <p className="text-sm text-destructive">{error}</p> : null}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-3 pb-2">
          <CardTitle className="text-base">GitHub</CardTitle>
          <Badge variant={connected ? "default" : "secondary"}>
            {github?.status ?? "disconnected"}
          </Badge>
        </CardHeader>
        <CardContent className="space-y-4 text-sm text-muted-foreground">
          {connected && github?.metadata?.login ? (
            <p>
              Connected as <span className="font-medium text-foreground">@{github.metadata.login}</span>
              {github.metadata.name ? ` (${github.metadata.name})` : null}
            </p>
          ) : (
            <p>
              Connect GitHub to let Bots list repositories, read files, and inspect issues and pull requests
              through read-only tools.
            </p>
          )}
          <div className="flex flex-wrap gap-2">
            {connected ? (
              <Button type="button" variant="outline" disabled={busy} onClick={handleDisconnect}>
                Disconnect
              </Button>
            ) : (
              <Button type="button" disabled={busy} onClick={handleConnect}>
                {busy ? "Starting…" : "Connect GitHub"}
              </Button>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
