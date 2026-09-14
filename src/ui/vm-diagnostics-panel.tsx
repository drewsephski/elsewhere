import { useCallback, useEffect, useState } from "react";
import { Alert, AlertDescription } from "@desktop/components/ui/alert";
import { Button } from "@desktop/components/ui/button";
import { Input } from "@desktop/components/ui/input";
import {
  guestResponseSchema,
  vmInfoSchema,
  vmService,
  type GuestResponse,
  type VmInfo,
} from "@desktop/services/vm-service";
import { LoaderCircle } from "@desktop/components/icons/lucide";
import { cn } from "@desktop/lib/utils";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const mib = bytes / (1024 * 1024);
  if (mib < 1024) return `${mib.toFixed(1)} MiB`;
  return `${(mib / 1024).toFixed(2)} GiB`;
}

interface VmDiagnosticsPanelProps {
  open: boolean;
  embedded?: boolean;
}

export function VmDiagnosticsPanel({ open, embedded = false }: VmDiagnosticsPanelProps) {
  const [info, setInfo] = useState<VmInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [command, setCommand] = useState(
    'mkdir -p /workspace && echo "hello from the persistent Elsewhere computer" > /workspace/proof.txt',
  );
  const [lastResponse, setLastResponse] = useState<GuestResponse | null>(null);

  const refresh = useCallback(async () => {
    try {
      const data = await vmService.info();
      setInfo(vmInfoSchema.parse(data));
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    if (open) {
      void refresh();
    }
  }, [open, refresh]);

  async function runAction(action: () => Promise<VmInfo>) {
    setBusy(true);
    setError(null);
    try {
      const data = await action();
      setInfo(vmInfoSchema.parse(data));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleExec() {
    setBusy(true);
    setError(null);
    try {
      const response = await vmService.guestRequest({
        id: crypto.randomUUID(),
        method: "exec",
        params: { command },
      });
      setLastResponse(guestResponseSchema.parse(response));
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleReadProof() {
    setCommand("cat /workspace/proof.txt");
    setBusy(true);
    setError(null);
    try {
      const response = await vmService.guestRequest({
        id: crypto.randomUUID(),
        method: "exec",
        params: { command: "cat /workspace/proof.txt" },
      });
      setLastResponse(guestResponseSchema.parse(response));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  if (!open && !embedded) return null;

  if (embedded) {
    return (
      <VmSidebarControls
        info={info}
        error={error}
        busy={busy}
        onRefresh={() => void refresh()}
        onStart={() => void runAction(() => vmService.start())}
        onStop={() => void runAction(() => vmService.stop())}
        onRestart={() => void runAction(() => vmService.restart())}
      />
    );
  }

  return (
    <section
      className="border-t border-border/60 bg-muted/20 px-3 py-3 sm:px-4"
      aria-label="Agent Computer VM diagnostics"
    >
      <div className="mx-auto flex w-full max-w-3xl flex-col gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 className="text-sm font-medium text-foreground">
            Agent Computer — VM diagnostics (Phase 2A spike)
          </h2>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => void refresh()}
            disabled={busy}
          >
            Refresh
          </Button>
        </div>

        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        {info && (
          <dl className="grid grid-cols-1 gap-1 text-xs text-muted-foreground sm:grid-cols-2">
            <div>
              <dt className="font-medium text-foreground">Status</dt>
              <dd>{info.state}</dd>
            </div>
            <div>
              <dt className="font-medium text-foreground">Bridge</dt>
              <dd>{info.guestBridgeReady ? "ready" : "not ready"}</dd>
            </div>
            <div>
              <dt className="font-medium text-foreground">Host arch</dt>
              <dd>{info.hostArch}</dd>
            </div>
            <div>
              <dt className="font-medium text-foreground">Guest arch</dt>
              <dd>{info.guestArch}</dd>
            </div>
            <div>
              <dt className="font-medium text-foreground">CPU / RAM</dt>
              <dd>
                {info.cpuCount} vCPU · {info.memoryMib} MiB
              </dd>
            </div>
            <div>
              <dt className="font-medium text-foreground">Disk</dt>
              <dd className="break-all">
                {formatBytes(info.diskBytes)} — {info.diskPath}
              </dd>
            </div>
            {info.consoleLogPath ? (
              <div className="sm:col-span-2">
                <dt className="font-medium text-foreground">Guest console log</dt>
                <dd className="break-all font-mono">{info.consoleLogPath}</dd>
              </div>
            ) : null}
            {info.vmmBinaryPath ? (
              <div className="sm:col-span-2">
                <dt className="font-medium text-foreground">VMM binary</dt>
                <dd className="break-all font-mono">{info.vmmBinaryPath}</dd>
              </div>
            ) : null}
          </dl>
        )}

        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            size="sm"
            disabled={busy}
            onClick={() => void runAction(() => vmService.provision())}
          >
            Provision
          </Button>
          <Button
            type="button"
            size="sm"
            disabled={busy}
            onClick={() => void runAction(() => vmService.start())}
          >
            Start
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={busy}
            onClick={() => void runAction(() => vmService.stop())}
          >
            Stop
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={busy}
            onClick={() => void runAction(() => vmService.restart())}
          >
            Restart
          </Button>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            disabled={busy}
            onClick={() => void handleReadProof()}
          >
            Read proof.txt
          </Button>
        </div>

        <div className="flex flex-col gap-2 sm:flex-row">
          <Input
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            aria-label="Guest shell command"
            className="font-mono text-xs"
          />
          <Button type="button" size="sm" disabled={busy} onClick={() => void handleExec()}>
            Run in guest
          </Button>
        </div>

        {lastResponse && (
          <pre className="max-h-40 overflow-auto rounded-md border border-border/60 bg-background p-2 text-xs">
            {JSON.stringify(lastResponse, null, 2)}
          </pre>
        )}
      </div>
    </section>
  );
}

interface VmSidebarControlsProps {
  info: VmInfo | null;
  error: string | null;
  busy: boolean;
  onRefresh: () => void;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
}

function vmStateTone(state: string | undefined): string {
  if (!state) {
    return "bg-muted-foreground/40";
  }
  const lower = state.toLowerCase();
  if (lower.includes("run") || lower.includes("start")) {
    return "bg-emerald-500";
  }
  if (lower.includes("stop") || lower.includes("off")) {
    return "bg-zinc-400";
  }
  return "bg-amber-500";
}

function VmSidebarControls({
  info,
  error,
  busy,
  onRefresh,
  onStart,
  onStop,
  onRestart,
}: VmSidebarControlsProps) {
  const statusLabel = info?.state ?? (error ? "Unavailable" : "Checking…");
  const meta =
    info &&
    `${info.guestBridgeReady ? "Bridge on" : "Bridge off"} · ${info.cpuCount} vCPU`;

  return (
    <div className="flex flex-col gap-2 px-1 pb-1 pt-0.5" aria-label="Agent computer controls">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span
              className={cn("size-1.5 shrink-0 rounded-full", vmStateTone(info?.state))}
              aria-hidden
            />
            <span className="truncate text-[10px] font-medium text-foreground">
              {statusLabel}
            </span>
            {busy && (
              <LoaderCircle
                className="size-3 shrink-0 animate-spin text-muted-foreground"
                aria-label="Working"
              />
            )}
          </div>
          {meta && (
            <p className="mt-0.5 truncate pl-3 text-[9px] text-muted-foreground">{meta}</p>
          )}
          {error && (
            <p
              className="mt-1 line-clamp-2 pl-3 text-[9px] leading-snug text-destructive/90"
              title={error}
            >
              {error}
            </p>
          )}
        </div>
        <Button
          type="button"
          variant="ghost"
          size="xs"
          className="h-6 shrink-0 px-1.5 text-[9px] text-muted-foreground"
          onClick={onRefresh}
          disabled={busy}
        >
          Refresh
        </Button>
      </div>

      <div className="flex flex-wrap gap-1">
        <SidebarAction disabled={busy} onClick={onStart}>
          Start
        </SidebarAction>
        <SidebarAction disabled={busy} onClick={onStop}>
          Stop
        </SidebarAction>
        <SidebarAction disabled={busy} onClick={onRestart}>
          Restart
        </SidebarAction>
      </div>
    </div>
  );
}

function SidebarAction({
  children,
  disabled,
  onClick,
}: {
  children: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <Button
      type="button"
      variant="outline"
      size="xs"
      disabled={disabled}
      className="h-6 border-border/50 bg-white/50 px-2 text-[9px] font-normal text-foreground shadow-none hover:bg-white"
      onClick={onClick}
    >
      {children}
    </Button>
  );
}
