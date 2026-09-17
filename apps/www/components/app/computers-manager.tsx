"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { ComputerSummary } from "@/lib/api-types";
import { computerProviderLabel, isLocalMacProvider } from "@/lib/computer-kind";
import { WorkspaceDataGrid } from "@/components/app/workspace-data-grid";
import { WorkspaceEmptyState } from "@/components/app/workspace-empty-state";
import {
  dataGridFeatures,
  type DataGridFeatures,
} from "@/components/reui/data-grid/data-grid";
import { Badge } from "@/components/reui/badge";
import {
  Frame,
  FrameDescription,
  FrameHeader,
  FramePanel,
  FrameTitle,
} from "@/components/reui/frame";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Button } from "@/components/ui/button";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  ColumnDef,
  PaginationState,
  useTable,
} from "@tanstack/react-table";
import { ConfirmAlertDialog } from "@/components/app/confirm-alert-dialog";
import { Monitor } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

type ComputerConfirmAction =
  | { kind: "reset-browser"; computerId: string }
  | { kind: "archive"; computerId: string };

export function ComputersManager() {
  const [computers, setComputers] = useState<ComputerSummary[]>([]);
  const [displayName, setDisplayName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });
  const [confirmAction, setConfirmAction] = useState<ComputerConfirmAction | null>(
    null,
  );

  const load = useCallback(async () => {
    try {
      const response = await cloudHostFetch("/v1/computers");
      if (!response.ok) throw new Error("Could not load computers");
      setComputers((await response.json()) as ComputerSummary[]);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load computers");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleCreate(event: React.FormEvent) {
    event.preventDefault();
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const response = await cloudHostFetch("/v1/computers", {
        method: "POST",
        body: JSON.stringify({ displayName }),
      });
      if (!response.ok) throw new Error("Could not create computer");
      setDisplayName("");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not create computer");
    } finally {
      setBusy(false);
    }
  }

  const handleConfirmAction = useCallback(async () => {
    if (!confirmAction || busy) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      if (confirmAction.kind === "reset-browser") {
        const response = await cloudHostFetch(
          `/v1/computers/${encodeURIComponent(confirmAction.computerId)}/browser-profile/reset`,
          { method: "POST" },
        );
        if (!response.ok) {
          const body = await response.json().catch(() => ({}));
          throw new Error(
            (body as { error?: string }).error ?? "Could not reset browser sign-in",
          );
        }
      } else {
        const response = await cloudHostFetch(
          `/v1/computers/${confirmAction.computerId}`,
          { method: "DELETE" },
        );
        const body = response.ok ? null : await response.json();
        if (!response.ok) {
          throw new Error(body.error ?? "Could not archive computer");
        }
        await load();
      }
      setConfirmAction(null);
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : confirmAction.kind === "reset-browser"
            ? "Could not reset browser sign-in"
            : "Could not archive computer",
      );
    } finally {
      setBusy(false);
    }
  }, [busy, confirmAction, load]);

  const confirmDialogCopy = useMemo(() => {
    if (!confirmAction) {
      return null;
    }
    if (confirmAction.kind === "reset-browser") {
      return {
        title: "Reset browser sign-in?",
        description:
          "Saved logins (Gmail, GitHub, etc.) will be cleared for this computer. You can sign in again via take control.",
        confirmLabel: "Reset",
        pendingLabel: "Resetting…",
        destructive: false,
      };
    }
    return {
      title: "Archive this computer?",
      description:
        "Its files will be preserved. Finish or stop its work before archiving.",
      confirmLabel: "Archive",
      pendingLabel: "Archiving…",
      destructive: true,
    };
  }, [confirmAction]);

  const columns = useMemo<ColumnDef<DataGridFeatures, ComputerSummary>[]>(
    () => [
      {
        accessorKey: "displayName",
        header: "Computer",
        cell: ({ row }) => (
          <div className="flex items-center gap-2">
            <Monitor className="size-4 shrink-0 text-muted-foreground" aria-hidden />
            <span className="font-medium">{row.original.displayName}</span>
          </div>
        ),
        size: 220,
      },
      {
        id: "kind",
        header: "Kind",
        cell: ({ row }) => (
          <span className="text-muted-foreground">
            {computerProviderLabel(row.original.provider)}
          </span>
        ),
        size: 120,
      },
      {
        id: "status",
        header: "Status",
        cell: ({ row }) => {
          const localMac = isLocalMacProvider(row.original.provider);
          const online = row.original.providerMetadata.connected === true;
          const provisioned = row.original.providerMetadata.provisioned;
          const label = localMac
            ? online
              ? "Online"
              : "Offline"
            : provisioned
              ? "Ready for work"
              : "Ready to set up";
          const variant = localMac
            ? online
              ? "success-light"
              : "warning-light"
            : provisioned
              ? "success-light"
              : "warning-light";
          return <Badge variant={variant}>{label}</Badge>;
        },
        size: 140,
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) => (
          <div className="flex justify-end gap-1">
            {isLocalMacProvider(row.original.provider) ? null : (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="text-muted-foreground"
              disabled={busy}
              onClick={() =>
                setConfirmAction({
                  kind: "reset-browser",
                  computerId: row.original.id,
                })
              }
            >
              Reset browser sign-in
            </Button>
            )}
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="text-muted-foreground"
              disabled={busy}
              onClick={() =>
                setConfirmAction({ kind: "archive", computerId: row.original.id })
              }
            >
              Archive
            </Button>
          </div>
        ),
        size: 220,
        enableSorting: false,
      },
    ],
    [busy],
  );

  const table = useTable({
    features: dataGridFeatures,
    columns,
    data: computers,
    pageCount: Math.ceil(computers.length / pagination.pageSize) || 1,
    getRowId: (row) => row.id,
    state: { pagination },
    onPaginationChange: setPagination,
  });

  return (
    <div className="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
      <div className="space-y-4">
        {error ? (
          <Alert variant="destructive">
            <AlertTitle>Could not update computers</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}
        <WorkspaceDataGrid
          table={table}
          recordCount={computers.length}
          loading={loading && computers.length === 0}
          emptyMessage={
            <WorkspaceEmptyState
              title="No computers yet"
              description="Each computer keeps its files between assignments. It will be prepared when your bot first needs it."
              icon={<Monitor aria-hidden />}
            />
          }
        />
      </div>

      <Frame className="w-full" spacing="sm">
        <FrameHeader>
          <FrameTitle>New computer</FrameTitle>
          <FrameDescription>
            Bots use computers to persist files and run delegated work.
          </FrameDescription>
        </FrameHeader>
        <FramePanel>
          <form onSubmit={(event) => void handleCreate(event)}>
            <FormFields>
            <FormItem>
              <Label htmlFor="computer-name">Display name</Label>
              <Input
                id="computer-name"
                required
                maxLength={100}
                value={displayName}
                onChange={(event) => setDisplayName(event.target.value)}
              />
            </FormItem>
            <Button type="submit" disabled={busy || !displayName.trim()}>
              {busy ? "Saving…" : "Create computer"}
            </Button>
            </FormFields>
          </form>
        </FramePanel>
      </Frame>

      {confirmDialogCopy ? (
        <ConfirmAlertDialog
          open={Boolean(confirmAction)}
          onOpenChange={(open) => {
            if (!open && !busy) {
              setConfirmAction(null);
            }
          }}
          title={confirmDialogCopy.title}
          description={confirmDialogCopy.description}
          confirmLabel={confirmDialogCopy.confirmLabel}
          pendingLabel={confirmDialogCopy.pendingLabel}
          destructive={confirmDialogCopy.destructive}
          pending={busy}
          onConfirm={handleConfirmAction}
        />
      ) : null}
    </div>
  );
}
