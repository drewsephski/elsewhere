"use client";

import { AuthShell } from "@/components/auth/auth-shell";
import { Button, buttonVariants } from "@/components/ui/button";
import { cloudHostFetch } from "@/lib/cloud-api";
import { appRoutes } from "@/lib/app-routes";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useEffect, useMemo, useState } from "react";

interface PairingView {
  pairingId: string;
  deviceName: string;
  expiresAt: string;
  status: string;
}

function formatExpiry(iso: string | null): string {
  if (!iso) {
    return "Unknown";
  }
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return "Unknown";
  }
  return date.toLocaleString();
}

export function PairMacView() {
  const searchParams = useSearchParams();
  const pairingId = searchParams.get("pairingId")?.trim() ?? "";
  const userCode = searchParams.get("userCode")?.trim() ?? "";
  const [pairing, setPairing] = useState<PairingView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    if (!pairingId) {
      setError("This pairing link is missing a Mac identifier.");
      return;
    }
    const controller = new AbortController();
    void (async () => {
      const response = await cloudHostFetch(
        `/v1/local-mac/pairings/${encodeURIComponent(pairingId)}`,
        { signal: controller.signal },
      );
      if (!response.ok) {
        setError("This pairing request expired or could not be found.");
        return;
      }
      setPairing((await response.json()) as PairingView);
      setError(null);
    })();
    return () => controller.abort();
  }, [pairingId]);

  const expired = useMemo(() => {
    if (!pairing?.expiresAt) {
      return false;
    }
    return Date.parse(pairing.expiresAt) <= Date.now() || pairing.status === "expired";
  }, [pairing]);

  async function handleApprove() {
    if (!pairingId || pending) {
      return;
    }
    setPending(true);
    setError(null);
    try {
      const response = await cloudHostFetch(
        `/v1/local-mac/pairings/${encodeURIComponent(pairingId)}/approve`,
        {
          method: "POST",
          body: JSON.stringify({ userCode }),
        },
      );
      if (!response.ok) {
        const body = (await response.json().catch(() => null)) as { error?: string } | null;
        throw new Error(body?.error ?? "Could not approve this Mac.");
      }
      setConnected(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not approve this Mac.");
    } finally {
      setPending(false);
    }
  }

  if (connected) {
    return (
      <AuthShell
        title="This Mac is connected"
        description="This Mac is connected to your Elsewhere account. You can return to the app."
      >
        <Link href={appRoutes.computers} className={`${buttonVariants()} mt-6 w-full`}>
          Open Computers
        </Link>
      </AuthShell>
    );
  }

  return (
    <AuthShell
      title="Connect this Mac"
      description="Approve this computer so it can appear as This Mac in your Elsewhere workspace."
    >
      <dl className="mt-6 space-y-3 text-sm">
        <div>
          <dt className="text-muted-foreground">Device</dt>
          <dd className="font-medium text-foreground">{pairing?.deviceName ?? "This Mac"}</dd>
        </div>
        <div>
          <dt className="text-muted-foreground">Code</dt>
          <dd className="font-mono text-base tracking-wide text-foreground">
            {userCode || "Open this link from the Mac app"}
          </dd>
        </div>
        <div>
          <dt className="text-muted-foreground">Expires</dt>
          <dd className="text-foreground">{formatExpiry(pairing?.expiresAt ?? null)}</dd>
        </div>
      </dl>
      {error ? (
        <p className="mt-4 text-sm text-destructive" role="alert">
          {error}
        </p>
      ) : null}
      <Button
        type="button"
        className="mt-6 w-full"
        disabled={pending || !pairingId || !userCode || expired}
        onClick={() => void handleApprove()}
      >
        {pending ? "Connecting…" : "Approve"}
      </Button>
    </AuthShell>
  );
}
