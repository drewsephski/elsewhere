import { isTauriRuntime } from "@/lib/tauri-runtime";
import { Button } from "@/components/ui/button";
import { tauriApi } from "@desktop/lib/tauri-api";
import { useCallback, useEffect, useState } from "react";

export function MacCompanionConnect() {
  const [ready, setReady] = useState(false);
  const [connected, setConnected] = useState(false);
  const [reauthRequired, setReauthRequired] = useState(false);
  const [pairing, setPairing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }
    setReady(true);
    void tauriApi
      .getElsewherePairingStatus()
      .then((status) => {
        setConnected(status.connected);
        setReauthRequired(status.reauthRequired);
      })
      .catch((err) => {
        setError(err instanceof Error ? err.message : "Could not read Mac pairing status");
      });
  }, []);

  const handleConnect = useCallback(async () => {
    setPairing(true);
    setError(null);
    try {
      const status = await tauriApi.startElsewherePairing();
      setConnected(status.connected);
      setReauthRequired(status.reauthRequired);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not start pairing");
    } finally {
      setPairing(false);
    }
  }, []);

  if (!ready) {
    return null;
  }

  return (
    <div className="flex flex-col items-stretch gap-2 sm:items-end">
      <Button
        type="button"
        variant={connected && !reauthRequired ? "outline" : "default"}
        disabled={pairing}
        onClick={() => void handleConnect()}
        aria-label={
          pairing
            ? "Waiting for Mac pairing approval"
            : reauthRequired
              ? "Reconnect this Mac to Elsewhere"
              : connected
                ? "Reconnect this Mac to Elsewhere"
                : "Connect this Mac to Elsewhere"
        }
      >
        {pairing
          ? "Waiting for approval…"
          : reauthRequired
            ? "Reconnect this Mac"
            : connected
              ? "Reconnect this Mac"
              : "Connect this Mac"}
      </Button>
      {error ? (
        <p className="max-w-xs text-right text-xs text-destructive" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
