"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { ComputerSummary } from "@/lib/api-types";
import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export function ComputersManager() {
  const [computers, setComputers] = useState<ComputerSummary[]>([]);
  const [displayName, setDisplayName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const response = await cloudHostFetch("/v1/computers");
      if (!response.ok) throw new Error("Could not load computers");
      setComputers((await response.json()) as ComputerSummary[]);
    } catch (err) { setError(err instanceof Error ? err.message : "Could not load computers"); }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleCreate(event: React.FormEvent) {
    event.preventDefault();
    if (busy) return;
    setBusy(true); setError(null);
    try {
      const response = await cloudHostFetch("/v1/computers", { method: "POST", body: JSON.stringify({ displayName }) });
      if (!response.ok) throw new Error("Could not create computer");
      setDisplayName(""); await load();
    } catch (err) { setError(err instanceof Error ? err.message : "Could not create computer"); }
    finally { setBusy(false); }
  }

  async function handleArchive(id: string) {
    if (!window.confirm("Archive this computer? Its files will be preserved. Finish or stop its work first.")) {
      return;
    }
    if (busy) return;
    setBusy(true); setError(null);
    try {
      const response = await cloudHostFetch(`/v1/computers/${id}`, { method: "DELETE" });
      const body = response.ok ? null : await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not archive computer");
      await load();
    } catch (err) { setError(err instanceof Error ? err.message : "Could not archive computer"); }
    finally { setBusy(false); }
  }

  return (
    <div className="grid gap-8 lg:grid-cols-[1fr_280px]">
      <section className="surface-card">
        <h2 className="text-sm font-medium uppercase tracking-[0.15em]">Computers</h2>
        <p className="mt-2 text-xs text-brand-dark/55">
          Each computer keeps its files between assignments. It will be prepared when your bot first needs it.
        </p>
        <ul className="mt-4 space-y-3">
          {computers.map((computer) => (
            <li key={computer.id} className="flex items-start justify-between gap-3 border border-border rounded-lg p-3">
              <div>
                <p className="font-medium">{computer.displayName}</p>
                <p className="mt-1 text-xs text-brand-dark/55">
                  {computer.providerMetadata.provisioned ? "Ready for work" : "Ready to set up"}
                </p>
              </div>
              <button
                type="button"
                disabled={busy}
                onClick={() => void handleArchive(computer.id)}
                className="text-xs text-brand-dark/50 underline-offset-4 hover:underline"
              >
                Archive
              </button>
            </li>
          ))}
          {computers.length === 0 ? (
            <li className="text-sm text-brand-dark/50">No computers yet.</li>
          ) : null}
        </ul>
      </section>

      <section className="surface-card">
        <h2 className="text-sm font-medium uppercase tracking-[0.15em]">New computer</h2>
        <form className="mt-4 space-y-3" onSubmit={(e) => void handleCreate(e)}>
          <div>
            <Label htmlFor="computer-name">Display name</Label>
            <Input
              id="computer-name"
              required
              maxLength={100}
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
            />
          </div>
          {error ? (
            <p className="text-sm text-red-700" role="alert">
              {error}
            </p>
          ) : null}
          <Button type="submit" disabled={busy || !displayName.trim()}>{busy ? "Saving…" : "Create computer"}</Button>
        </form>
      </section>
    </div>
  );
}
