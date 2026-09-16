"use client";

import { useCallback, useEffect, useState } from "react";
import { cloudHostErrorMessage, cloudHostFetch } from "@/lib/cloud-api";
import { ConfirmAlertDialog } from "@/components/app/confirm-alert-dialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
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
import { toast } from "sonner";

interface GithubSummary {
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

type AddKind = "mcp" | "openapi";

export function ConnectorsManager() {
  const [github, setGithub] = useState<GithubSummary | null>(null);
  const [installs, setInstalls] = useState<InstallSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [addKind, setAddKind] = useState<AddKind | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<InstallSummary | null>(null);

  const load = useCallback(async () => {
    const [githubResponse, installsResponse] = await Promise.all([
      cloudHostFetch("/v1/connectors/github"),
      cloudHostFetch("/v1/connectors/installs"),
    ]);
    if (!githubResponse.ok) {
      setError("Could not load GitHub connection");
      return;
    }
    setGithub((await githubResponse.json()) as GithubSummary);
    if (installsResponse.ok) {
      setInstalls((await installsResponse.json()) as InstallSummary[]);
    }
    setError(null);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleConnectGithub() {
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

  const githubConnected = github?.status === "connected";

  return (
    <div className="space-y-4">
      {error ? <p className="text-sm text-destructive">{error}</p> : null}
      <div className="flex justify-end">
        <Button type="button" onClick={() => setAddOpen(true)}>
          Add integration
        </Button>
      </div>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-3 pb-2">
          <CardTitle className="text-base">GitHub</CardTitle>
          <Badge variant={githubConnected ? "default" : "secondary"}>
            {github?.status ?? "disconnected"}
          </Badge>
        </CardHeader>
        <CardContent className="space-y-4 text-sm text-muted-foreground">
          {githubConnected && github?.metadata?.login ? (
            <p>
              Connected as{" "}
              <span className="font-medium text-foreground">@{github.metadata.login}</span>
              {github.metadata.name ? ` (${github.metadata.name})` : null}
            </p>
          ) : (
            <p>
              Connect GitHub to let Bots list repositories, read files, and inspect issues and pull
              requests through read-only tools.
            </p>
          )}
          <div className="flex flex-wrap gap-2">
            {githubConnected ? (
              <Button type="button" variant="outline" disabled={busy} onClick={handleDisconnectGithub}>
                Disconnect
              </Button>
            ) : (
              <Button type="button" disabled={busy} onClick={handleConnectGithub}>
                {busy ? "Starting…" : "Connect GitHub"}
              </Button>
            )}
          </div>
        </CardContent>
      </Card>

      {installs.map((install) => (
        <Card key={install.id}>
          <CardHeader className="flex flex-row items-center justify-between gap-3 pb-2">
            <CardTitle className="text-base">{install.displayName}</CardTitle>
            <Badge variant={install.status === "connected" && install.enabled ? "default" : "secondary"}>
              {install.enabled ? install.status.replaceAll("_", " ") : "disabled"}
            </Badge>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-muted-foreground">
            <p>
              {install.kind === "mcp" ? "MCP server" : "OpenAPI API"} · {install.toolCount}{" "}
              {install.toolCount === 1 ? "tool" : "tools"}
            </p>
            {install.sampleTools.length > 0 ? (
              <p className="text-foreground/80">{install.sampleTools.join(", ")}</p>
            ) : null}
            {install.lastDiscoveryError ? (
              <p className="text-destructive">{install.lastDiscoveryError}</p>
            ) : null}
            <div className="flex flex-wrap gap-2">
              {install.status === "reconnect_required" ? (
                <Button type="button" size="sm" disabled={busy} onClick={() => void handleReconnect(install)}>
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
            </div>
          </CardContent>
        </Card>
      ))}

      <Dialog open={addOpen} onOpenChange={(open) => {
        setAddOpen(open);
        if (!open) setAddKind(null);
      }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Add integration</DialogTitle>
            <DialogDescription>
              Connect an MCP server or OpenAPI API so your Bots can use its tools.
            </DialogDescription>
          </DialogHeader>
          {addKind === null ? (
            <div className="grid gap-2">
              <Button type="button" variant="outline" onClick={() => setAddKind("mcp")}>
                MCP Server
              </Button>
              <Button type="button" variant="outline" onClick={() => setAddKind("openapi")}>
                OpenAPI API
              </Button>
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
        description="Your Bots will no longer be able to use its tools. This does not affect GitHub."
        confirmLabel="Delete"
        destructive
        pending={busy}
        onConfirm={() => void handleDelete()}
      />
    </div>
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
  const [auth, setAuth] = useState(kind === "mcp" ? "none" : "none");
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
