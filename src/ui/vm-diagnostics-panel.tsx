import { useCallback, useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  guestResponseSchema,
  vmInfoSchema,
  vmService,
  type GuestResponse,
  type VmInfo,
} from "@/services/vm-service";

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
    'mkdir -p /workspace && echo "hello from the persistent GPT Bot computer" > /workspace/proof.txt',
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

  return (
    <section
      className={
        embedded
          ? "flex flex-col gap-3"
          : "border-t border-border/60 bg-muted/20 px-3 py-3 sm:px-4"
      }
      aria-label="Agent Computer VM diagnostics"
    >
      <div
        className={
          embedded
            ? "flex w-full flex-col gap-3"
            : "mx-auto flex w-full max-w-3xl flex-col gap-3"
        }
      >
        {!embedded && (
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
        )}

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
