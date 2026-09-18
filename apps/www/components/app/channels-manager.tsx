"use client";

import { useCallback, useEffect, useState } from "react";
import { cloudHostErrorMessage, cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary } from "@/lib/api-types";
import { ConfirmAlertDialog } from "@/components/app/confirm-alert-dialog";
import { BotSelect } from "@/components/app/bot-select";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { SlackLogo } from "@/components/icons/slack-logo";
import { toast } from "sonner";

interface ChannelConnectionSummary {
  id: string;
  provider: string;
  status: string;
  enabled: boolean;
  workspaceName?: string | null;
  defaultBotId?: string | null;
  defaultBotName?: string | null;
  connectedAt?: string | null;
  updatedAt: string;
}

export function ChannelsManager() {
  const [connections, setConnections] = useState<ChannelConnectionSummary[]>([]);
  const [bots, setBots] = useState<BotSummary[]>([]);
  const [botId, setBotId] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [disconnectTarget, setDisconnectTarget] =
    useState<ChannelConnectionSummary | null>(null);

  const slack = connections.find((row) => row.provider === "slack") ?? null;
  const connected = slack?.status === "connected";

  const load = useCallback(async () => {
    const [channelsResponse, botsResponse] = await Promise.all([
      cloudHostFetch("/v1/channels"),
      cloudHostFetch("/v1/bots"),
    ]);
    if (!channelsResponse.ok) {
      setError("Could not load channels");
      return;
    }
    const rows = (await channelsResponse.json()) as ChannelConnectionSummary[];
    setConnections(rows);
    if (botsResponse.ok) {
      const botRows = (await botsResponse.json()) as BotSummary[];
      setBots(botRows);
      setBotId((current) => {
        if (current && botRows.some((bot) => bot.id === current)) {
          return current;
        }
        const connectedBot = rows.find((row) => row.provider === "slack")
          ?.defaultBotId;
        if (connectedBot && botRows.some((bot) => bot.id === connectedBot)) {
          return connectedBot;
        }
        return botRows[0]?.id ?? "";
      });
    }
    setError(null);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleConnect() {
    if (!botId) {
      toast.error("Choose a Bot first");
      return;
    }
    setBusy(true);
    try {
      const response = await cloudHostFetch("/v1/channels/slack/oauth/start", {
        method: "POST",
        body: JSON.stringify({ botId }),
      });
      if (!response.ok) {
        throw new Error(await cloudHostErrorMessage(response, "Could not start Slack"));
      }
      const body = (await response.json()) as { authorizeUrl: string };
      window.location.assign(body.authorizeUrl);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not start Slack");
      setBusy(false);
    }
  }

  async function handleChangeBot(nextBotId: string) {
    if (!slack || slack.status !== "connected") {
      setBotId(nextBotId);
      return;
    }
    setBotId(nextBotId);
    setBusy(true);
    try {
      const response = await cloudHostFetch(`/v1/channels/${slack.id}`, {
        method: "PATCH",
        body: JSON.stringify({ defaultBotId: nextBotId }),
      });
      if (!response.ok) {
        throw new Error(await cloudHostErrorMessage(response, "Could not change Bot"));
      }
      toast.success("Slack Bot updated");
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not change Bot");
    } finally {
      setBusy(false);
    }
  }

  async function handleDisconnect() {
    if (!disconnectTarget) {
      return;
    }
    setBusy(true);
    try {
      const response = await cloudHostFetch(`/v1/channels/${disconnectTarget.id}`, {
        method: "DELETE",
      });
      if (!response.ok) {
        throw new Error(await cloudHostErrorMessage(response, "Could not disconnect Slack"));
      }
      toast.success("Slack disconnected");
      setDisconnectTarget(null);
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not disconnect Slack");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-4">
      {error ? (
        <p className="text-sm text-destructive" role="alert">
          {error}
        </p>
      ) : null}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle className="flex items-center gap-2.5 text-base">
            <span className="flex size-8 items-center justify-center rounded-lg bg-background ring-1 ring-foreground/10">
              <SlackLogo className="size-4" />
            </span>
            Slack
          </CardTitle>
          <Badge variant={connected ? "default" : "secondary"}>
            {connected ? "Connected" : "Disconnected"}
          </Badge>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            DM the Elsewhere Slack app or @mention it in Slack to talk to this
            Bot.
          </p>
          {connected && slack?.workspaceName ? (
            <p className="text-sm">
              Workspace: <span className="font-medium">{slack.workspaceName}</span>
            </p>
          ) : null}
          <BotSelect
            id="slack-bot"
            label="Elsewhere Bot"
            value={botId}
            onValueChange={(next) => void handleChangeBot(next)}
            bots={bots}
            disabled={busy || bots.length === 0}
            required
          />
          <div className="flex flex-wrap gap-2">
            {connected ? (
              <Button
                type="button"
                variant="outline"
                disabled={busy}
                onClick={() => setDisconnectTarget(slack)}
              >
                Disconnect
              </Button>
            ) : (
              <Button
                type="button"
                disabled={busy || !botId}
                onClick={() => void handleConnect()}
              >
                Connect Slack
              </Button>
            )}
          </div>
        </CardContent>
      </Card>
      <ConfirmAlertDialog
        open={disconnectTarget !== null}
        onOpenChange={(open) => {
          if (!open) {
            setDisconnectTarget(null);
          }
        }}
        title="Disconnect Slack?"
        description="Elsewhere will stop listening to this Slack workspace. You can connect again later."
        confirmLabel="Disconnect"
        pending={busy}
        onConfirm={() => void handleDisconnect()}
      />
    </div>
  );
}
