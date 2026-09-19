"use client";

import { useCallback, useEffect, useState, type ReactNode } from "react";
import { cloudHostErrorMessage, cloudHostFetch } from "@/lib/cloud-api";
import { appRoutes } from "@/lib/app-routes";
import { rememberSlackOAuthReturn } from "@/lib/slack-oauth-return";
import { startGithubConnectorOAuth } from "@/lib/github-oauth";
import type { BotSummary } from "@/lib/api-types";
import { ConfirmAlertDialog } from "@/components/app/confirm-alert-dialog";
import { IntegrationCard } from "@/components/app/integration-card";
import { BotSelect } from "@/components/app/bot-select";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { GitHubLogo } from "@/components/icons/github-logo";
import { SlackLogo } from "@/components/icons/slack-logo";
import { McpLogo } from "@/components/icons/mcp-logo";
import { OpenApiLogo } from "@/components/icons/openapi-logo";
import { Plus } from "@/components/icons/lucide";
import type { StatusTone } from "@/components/app/status-pill";
import { toast } from "sonner";

interface GithubInstallationSummary {
  id: number;
  accountLogin: string;
  accountId: number;
  accountType: string;
  repositorySelection: string;
}

interface GithubSummary {
  provider: string;
  status: string;
  metadata: {
    login?: string;
    name?: string;
    avatarUrl?: string;
    githubUser?: {
      login?: string;
      userId?: number;
      name?: string;
      avatarUrl?: string;
    };
    installations?: GithubInstallationSummary[];
    authorizedRepositoryCount?: number;
  };
  connectedAt?: string | null;
  updatedAt: string;
}

interface InstallSummary {
  id: string;
  kind: "mcp" | "openapi";
  displayName: string;
  endpointUrl: string;
  status: string;
  enabled: boolean;
  toolCount: number;
  sampleTools: string[];
  lastDiscoveryError?: string | null;
  createdAt: string;
  updatedAt: string;
}

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

type AddKind = "mcp" | "openapi";

function connectionStatus(connected: boolean): { label: string; tone: StatusTone } {
  return connected
    ? { label: "Connected", tone: "success" }
    : { label: "Not connected", tone: "neutral" };
}

function githubCardStatus(github: GithubSummary | null): { label: string; tone: StatusTone } {
  if (github?.status === "connected") {
    return { label: "Connected", tone: "success" };
  }
  if (github?.status === "reconnect_required") {
    return { label: "Reconnect", tone: "warning" };
  }
  return { label: "Not connected", tone: "neutral" };
}

function githubUserLogin(github: GithubSummary | null): string | undefined {
  return github?.metadata?.githubUser?.login ?? github?.metadata?.login;
}

function githubInstalledOn(installations?: GithubInstallationSummary[]): string | null {
  if (!installations?.length) {
    return null;
  }
  return installations
    .map((install) => {
      if (install.accountType === "Organization") {
        return install.accountLogin;
      }
      return install.accountLogin
        ? `personal account (@${install.accountLogin})`
        : "personal account";
    })
    .join(", ");
}

function installStatus(install: InstallSummary): { label: string; tone: StatusTone } {
  if (!install.enabled) {
    return { label: "Disabled", tone: "neutral" };
  }
  if (install.status === "connected") {
    return { label: "Connected", tone: "success" };
  }
  if (install.status === "reconnect_required") {
    return { label: "Reconnect", tone: "warning" };
  }
  return { label: install.status.replaceAll("_", " "), tone: "neutral" };
}

export function ConnectorsManager() {
  const [github, setGithub] = useState<GithubSummary | null>(null);
  const [installs, setInstalls] = useState<InstallSummary[]>([]);
  const [slack, setSlack] = useState<ChannelConnectionSummary | null>(null);
  const [bots, setBots] = useState<BotSummary[]>([]);
  const [botId, setBotId] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [addOpen, setAddOpen] = useState(false);
  const [addKind, setAddKind] = useState<AddKind | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<InstallSummary | null>(null);
  const [disconnectSlack, setDisconnectSlack] = useState(false);

  const load = useCallback(async () => {
    const [githubResponse, installsResponse, channelsResponse, botsResponse] =
      await Promise.all([
        cloudHostFetch("/v1/connectors/github"),
        cloudHostFetch("/v1/connectors/installs"),
        cloudHostFetch("/v1/channels"),
        cloudHostFetch("/v1/bots"),
      ]);
    if (!githubResponse.ok) {
      setError("Could not load GitHub connection");
      setLoading(false);
      return;
    }
    setGithub((await githubResponse.json()) as GithubSummary);
    if (installsResponse.ok) {
      setInstalls((await installsResponse.json()) as InstallSummary[]);
    }
    let slackRow: ChannelConnectionSummary | null = null;
    if (channelsResponse.ok) {
      const rows = (await channelsResponse.json()) as ChannelConnectionSummary[];
      slackRow = rows.find((row) => row.provider === "slack") ?? null;
      setSlack(slackRow);
    }
    if (botsResponse.ok) {
      const botRows = (await botsResponse.json()) as BotSummary[];
      setBots(botRows);
      setBotId((current) => {
        if (current && botRows.some((bot) => bot.id === current)) {
          return current;
        }
        const connectedBot = slackRow?.defaultBotId;
        if (connectedBot && botRows.some((bot) => bot.id === connectedBot)) {
          return connectedBot;
        }
        return botRows[0]?.id ?? "";
      });
    }
    setError(null);
    setLoading(false);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleConnectGithub() {
    setBusy(true);
    setError(null);
    try {
      const body = await startGithubConnectorOAuth();
      window.location.href = body.authorizeUrl;
    } catch (err) {
      setError(err instanceof Error ? err.message : "GitHub App is not available. Check host configuration.");
    } finally {
      setBusy(false);
    }
  }

  async function handleDisconnectGithub() {
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
      toast.success("GitHub disconnected");
      await load();
    } finally {
      setBusy(false);
    }
  }

  async function handleConnectSlack() {
    if (!botId) {
      toast.error("Choose a Bot first");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const response = await cloudHostFetch("/v1/channels/slack/oauth/start", {
        method: "POST",
        body: JSON.stringify({ botId }),
      });
      if (!response.ok) {
        throw new Error(await cloudHostErrorMessage(response, "Could not start Slack"));
      }
      const body = (await response.json()) as { authorizeUrl: string };
      rememberSlackOAuthReturn(appRoutes.connectors);
      window.location.assign(body.authorizeUrl);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not start Slack");
      setBusy(false);
    }
  }

  async function handleChangeSlackBot(nextBotId: string) {
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
      setError(err instanceof Error ? err.message : "Could not change Bot");
    } finally {
      setBusy(false);
    }
  }

  async function handleDisconnectSlack() {
    if (!slack) {
      return;
    }
    setBusy(true);
    try {
      const response = await cloudHostFetch(`/v1/channels/${slack.id}`, {
        method: "DELETE",
      });
      if (!response.ok) {
        throw new Error(await cloudHostErrorMessage(response, "Could not disconnect Slack"));
      }
      toast.success("Slack disconnected");
      setDisconnectSlack(false);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not disconnect Slack");
    } finally {
      setBusy(false);
    }
  }

  async function handleDisable(install: InstallSummary, enabled: boolean) {
    setBusy(true);
    try {
      const response = await cloudHostFetch(`/v1/connectors/installs/${install.id}/enabled`, {
        method: "POST",
        body: JSON.stringify({ enabled }),
      });
      if (!response.ok) {
        throw new Error(await cloudHostErrorMessage(response, "Could not update integration"));
      }
      toast.success(enabled ? "Integration enabled" : "Integration disabled");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update integration");
    } finally {
      setBusy(false);
    }
  }

  async function handleReconnect(install: InstallSummary) {
    setBusy(true);
    try {
      const response = await cloudHostFetch(
        `/v1/connectors/installs/${install.id}/oauth/start`,
        { method: "POST" },
      );
      if (!response.ok) {
        throw new Error(await cloudHostErrorMessage(response, "Could not start reconnect"));
      }
      const body = (await response.json()) as { authorizationUrl?: string };
      if (body.authorizationUrl) {
        window.location.href = body.authorizationUrl;
        return;
      }
      throw new Error("Reconnect did not return an authorization URL");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not reconnect");
      setBusy(false);
    }
  }

  async function handleDelete() {
    if (!deleteTarget) return;
    setBusy(true);
    try {
      const response = await cloudHostFetch(`/v1/connectors/installs/${deleteTarget.id}`, {
        method: "DELETE",
      });
      if (!response.ok) {
        throw new Error(await cloudHostErrorMessage(response, "Could not delete integration"));
      }
      toast.success("Integration removed");
      setDeleteTarget(null);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not delete integration");
    } finally {
      setBusy(false);
    }
  }

  function handleOpenAdd(kind: AddKind | null = null) {
    setAddKind(kind);
    setAddOpen(true);
  }

  const githubConnected = github?.status === "connected";
  const githubNeedsReconnect = github?.status === "reconnect_required";
  const githubLogin = githubUserLogin(github);
  const githubInstalled = githubInstalledOn(github?.metadata?.installations);
  const slackConnected = slack?.status === "connected";
  const githubStatus = githubCardStatus(github);
  const slackStatus = connectionStatus(slackConnected);

  return (
    <div className="space-y-8">
      <WorkspacePageHeader
        title="Integrations"
        description="Connect the apps your Bots work in — GitHub, Slack, MCP servers, and any OpenAPI."
        action={
          <Button type="button" onClick={() => handleOpenAdd()}>
            <Plus data-icon="inline-start" aria-hidden />
            Add integration
          </Button>
        }
      />

      {error ? (
        <Alert variant="destructive">
          <AlertTitle>Could not update integrations</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {loading ? (
        <ul className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 4 }).map((_, index) => (
            <li key={index} className="rounded-xl p-3.5 ring-1 ring-foreground/10">
              <div className="flex items-start justify-between">
                <Skeleton className="size-10 rounded-xl" />
                <Skeleton className="h-5 w-20 rounded-full" />
              </div>
              <Skeleton className="mt-3 h-4 w-24" />
              <Skeleton className="mt-2 h-8 w-full" />
            </li>
          ))}
        </ul>
      ) : (
        <div className="space-y-8">
          <section aria-labelledby="available-apps-heading">
            <h2
              id="available-apps-heading"
              className="mb-3 text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground"
            >
              Apps
            </h2>
            <ul className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
              <li>
                <IntegrationCard
                  name="GitHub"
                  description="Let Bots list repositories, read files, and inspect issues and pull requests."
                  logo={<GitHubLogo className="size-5" />}
                  statusLabel={githubStatus.label}
                  statusTone={githubStatus.tone}
                  meta={
                    githubLogin || githubInstalled || github?.metadata?.authorizedRepositoryCount != null ? (
                      <div className="space-y-1">
                        {githubLogin ? (
                          <p>
                            Connected as{" "}
                            <span className="font-medium">@{githubLogin}</span>
                          </p>
                        ) : null}
                        {githubInstalled ? <p>Installed on: {githubInstalled}</p> : null}
                        {github?.metadata?.authorizedRepositoryCount != null ? (
                          <p>
                            Repositories: {github.metadata.authorizedRepositoryCount} authorized
                          </p>
                        ) : null}
                      </div>
                    ) : null
                  }
                  actions={
                    githubNeedsReconnect ? (
                      <>
                        <Button
                          type="button"
                          size="sm"
                          disabled={busy}
                          onClick={() => void handleConnectGithub()}
                        >
                          Reconnect GitHub
                        </Button>
                        <Button
                          type="button"
                          size="sm"
                          variant="outline"
                          disabled={busy}
                          onClick={() => void handleDisconnectGithub()}
                        >
                          Disconnect
                        </Button>
                      </>
                    ) : githubConnected ? (
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        disabled={busy}
                        onClick={() => void handleDisconnectGithub()}
                      >
                        Disconnect
                      </Button>
                    ) : (
                      <Button
                        type="button"
                        size="sm"
                        disabled={busy}
                        onClick={() => void handleConnectGithub()}
                      >
                        {busy ? "Starting…" : "Connect GitHub"}
                      </Button>
                    )
                  }
                />
              </li>
              <li>
                <IntegrationCard
                  name="Slack"
                  description="DM the Elsewhere app or @mention it in Slack to talk to a Bot."
                  logo={<SlackLogo className="size-5" />}
                  statusLabel={slackStatus.label}
                  statusTone={slackStatus.tone}
                  meta={
                    <>
                      {slackConnected && slack?.workspaceName ? (
                        <p className="mb-2">
                          Workspace:{" "}
                          <span className="font-medium">{slack.workspaceName}</span>
                        </p>
                      ) : null}
                      <BotSelect
                        id="slack-bot"
                        label="Elsewhere Bot"
                        value={botId}
                        onValueChange={(next) => void handleChangeSlackBot(next)}
                        bots={bots}
                        disabled={busy || bots.length === 0}
                        required
                      />
                    </>
                  }
                  actions={
                    slackConnected ? (
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        disabled={busy}
                        onClick={() => setDisconnectSlack(true)}
                      >
                        Disconnect
                      </Button>
                    ) : (
                      <Button
                        type="button"
                        size="sm"
                        disabled={busy || !botId}
                        onClick={() => void handleConnectSlack()}
                      >
                        Connect Slack
                      </Button>
                    )
                  }
                />
              </li>
              <li>
                <IntegrationCard
                  name="MCP Server"
                  description="Connect any Model Context Protocol server so Bots can use its tools."
                  logo={<McpLogo className="size-5" />}
                  statusLabel="Available"
                  statusTone="info"
                  actions={
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onClick={() => handleOpenAdd("mcp")}
                    >
                      Add MCP server
                    </Button>
                  }
                />
              </li>
              <li>
                <IntegrationCard
                  name="OpenAPI"
                  description="Import an API from an OpenAPI spec and expose its operations as tools."
                  logo={<OpenApiLogo className="size-5" />}
                  statusLabel="Available"
                  statusTone="info"
                  actions={
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onClick={() => handleOpenAdd("openapi")}
                    >
                      Add OpenAPI
                    </Button>
                  }
                />
              </li>
            </ul>
          </section>

          {installs.length > 0 ? (
            <section aria-labelledby="installed-apps-heading">
              <h2
                id="installed-apps-heading"
                className="mb-3 text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground"
              >
                Installed
              </h2>
              <ul className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
                {installs.map((install) => {
                  const status = installStatus(install);
                  return (
                    <li key={install.id}>
                      <IntegrationCard
                        name={install.displayName}
                        description={
                          install.kind === "mcp"
                            ? `MCP server · ${install.toolCount} ${install.toolCount === 1 ? "tool" : "tools"}`
                            : `OpenAPI API · ${install.toolCount} ${install.toolCount === 1 ? "tool" : "tools"}`
                        }
                        logo={
                          install.kind === "mcp" ? (
                            <McpLogo className="size-5" />
                          ) : (
                            <OpenApiLogo className="size-5" />
                          )
                        }
                        statusLabel={status.label}
                        statusTone={status.tone}
                        meta={
                          <>
                            {install.sampleTools.length > 0 ? (
                              <p className="line-clamp-2 text-foreground/80">
                                {install.sampleTools.join(", ")}
                              </p>
                            ) : null}
                            {install.lastDiscoveryError ? (
                              <p className="mt-1 text-destructive">{install.lastDiscoveryError}</p>
                            ) : null}
                          </>
                        }
                        actions={
                          <>
                            {install.status === "reconnect_required" ? (
                              <Button
                                type="button"
                                size="sm"
                                disabled={busy}
                                onClick={() => void handleReconnect(install)}
                              >
                                Reconnect
                              </Button>
                            ) : null}
                            <Button
                              type="button"
                              size="sm"
                              variant="outline"
                              disabled={busy}
                              onClick={() => void handleDisable(install, !install.enabled)}
                            >
                              {install.enabled ? "Disable" : "Enable"}
                            </Button>
                            <Button
                              type="button"
                              size="sm"
                              variant="ghost"
                              disabled={busy}
                              onClick={() => setDeleteTarget(install)}
                            >
                              Delete
                            </Button>
                          </>
                        }
                      />
                    </li>
                  );
                })}
              </ul>
            </section>
          ) : null}
        </div>
      )}

      <Dialog
        open={addOpen}
        onOpenChange={(open) => {
          setAddOpen(open);
          if (!open) setAddKind(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Add integration</DialogTitle>
            <DialogDescription>
              Connect an MCP server or OpenAPI API so your Bots can use its tools.
            </DialogDescription>
          </DialogHeader>
          {addKind === null ? (
            <div className="grid grid-cols-2 gap-2">
              <AddKindButton
                name="MCP Server"
                description="Tools from an MCP endpoint."
                logo={<McpLogo className="size-5" />}
                onClick={() => setAddKind("mcp")}
              />
              <AddKindButton
                name="OpenAPI"
                description="Operations from an API spec."
                logo={<OpenApiLogo className="size-5" />}
                onClick={() => setAddKind("openapi")}
              />
            </div>
          ) : (
            <AddIntegrationForm
              kind={addKind}
              busy={busy}
              onBusy={setBusy}
              onCancel={() => {
                setAddKind(null);
                setAddOpen(false);
              }}
              onCreated={async () => {
                setAddKind(null);
                setAddOpen(false);
                await load();
              }}
            />
          )}
        </DialogContent>
      </Dialog>

      <ConfirmAlertDialog
        open={Boolean(deleteTarget)}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
        title="Remove this integration?"
        description="Your Bots will no longer be able to use its tools. This does not affect GitHub or Slack."
        confirmLabel="Delete"
        destructive
        pending={busy}
        onConfirm={() => void handleDelete()}
      />

      <ConfirmAlertDialog
        open={disconnectSlack}
        onOpenChange={(open) => {
          if (!open) setDisconnectSlack(false);
        }}
        title="Disconnect Slack?"
        description="Elsewhere will stop listening to this Slack workspace. You can connect again later."
        confirmLabel="Disconnect"
        pending={busy}
        onConfirm={() => void handleDisconnectSlack()}
      />
    </div>
  );
}

function AddKindButton({
  name,
  description,
  logo,
  onClick,
}: {
  name: string;
  description: string;
  logo: ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex flex-col items-start gap-2 rounded-xl bg-muted/30 p-3 text-left ring-1 ring-foreground/10 transition-colors hover:bg-muted/50 hover:ring-foreground/16 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
    >
      <span className="flex size-9 items-center justify-center rounded-lg bg-background ring-1 ring-foreground/10">
        {logo}
      </span>
      <span className="text-sm font-medium">{name}</span>
      <span className="text-xs leading-4 text-muted-foreground">{description}</span>
    </button>
  );
}

function AddIntegrationForm({
  kind,
  busy,
  onBusy,
  onCancel,
  onCreated,
}: {
  kind: AddKind;
  busy: boolean;
  onBusy: (busy: boolean) => void;
  onCancel: () => void;
  onCreated: () => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [auth, setAuth] = useState("none");
  const [token, setToken] = useState("");
  const [headerName, setHeaderName] = useState("X-Api-Key");
  const [formError, setFormError] = useState<string | null>(null);

  async function handleSubmit() {
    onBusy(true);
    setFormError(null);
    try {
      if (kind === "mcp") {
        const response = await cloudHostFetch("/v1/connectors/installs/mcp", {
          method: "POST",
          body: JSON.stringify({
            name,
            serverUrl: url,
            auth,
            token: auth === "bearer" ? token : undefined,
          }),
        });
        const body = (await response.json().catch(() => ({}))) as {
          authorizationUrl?: string;
          error?: string;
        };
        if (!response.ok) {
          throw new Error(body.error ?? "Could not add MCP server");
        }
        if (body.authorizationUrl) {
          window.location.href = body.authorizationUrl;
          return;
        }
        toast.success("MCP server added");
      } else {
        const response = await cloudHostFetch("/v1/connectors/installs/openapi", {
          method: "POST",
          body: JSON.stringify({
            name,
            openapiUrl: url,
            auth,
            token: auth === "none" ? undefined : token,
            headerName: auth === "api_key_header" ? headerName : undefined,
          }),
        });
        if (!response.ok) {
          throw new Error(await cloudHostErrorMessage(response, "Could not add OpenAPI API"));
        }
        toast.success("OpenAPI API added");
      }
      await onCreated();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : "Could not add integration");
    } finally {
      onBusy(false);
    }
  }

  return (
    <form
      className="space-y-3"
      onSubmit={(event) => {
        event.preventDefault();
        void handleSubmit();
      }}
    >
      <div className="space-y-1.5">
        <Label htmlFor="integration-name">Name</Label>
        <Input
          id="integration-name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          required
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="integration-url">{kind === "mcp" ? "Server URL" : "OpenAPI URL"}</Label>
        <Input
          id="integration-url"
          value={url}
          onChange={(event) => setUrl(event.target.value)}
          placeholder="https://"
          required
        />
      </div>
      <div className="space-y-1.5">
        <Label>Authentication</Label>
        <Select value={auth} onValueChange={(value) => setAuth(String(value ?? "none"))}>
          <SelectTrigger aria-label="Authentication">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="none">None</SelectItem>
            <SelectItem value="bearer">Bearer token</SelectItem>
            {kind === "mcp" ? <SelectItem value="oauth">OAuth</SelectItem> : null}
            {kind === "openapi" ? (
              <SelectItem value="api_key_header">API key header</SelectItem>
            ) : null}
          </SelectContent>
        </Select>
      </div>
      {auth === "bearer" || auth === "api_key_header" ? (
        <div className="space-y-1.5">
          <Label htmlFor="integration-token">{auth === "bearer" ? "Token" : "API key"}</Label>
          <Input
            id="integration-token"
            type="password"
            value={token}
            onChange={(event) => setToken(event.target.value)}
            autoComplete="off"
          />
        </div>
      ) : null}
      {auth === "api_key_header" ? (
        <div className="space-y-1.5">
          <Label htmlFor="integration-header">Header name</Label>
          <Input
            id="integration-header"
            value={headerName}
            onChange={(event) => setHeaderName(event.target.value)}
          />
        </div>
      ) : null}
      {formError ? (
        <p className="text-sm text-destructive" role="alert">
          {formError}
        </p>
      ) : null}
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button type="submit" disabled={busy || !name.trim() || !url.trim()}>
          {busy ? "Testing…" : "Test / Add"}
        </Button>
      </DialogFooter>
    </form>
  );
}
