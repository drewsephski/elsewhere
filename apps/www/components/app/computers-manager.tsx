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
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const response = await cloudHostFetch("/v1/computers");
    if (!response.ok) {
      setError("Failed to load computers");
      return;
    }
    setComputers((await response.json()) as ComputerSummary[]);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleCreate(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    const response = await cloudHostFetch("/v1/computers", {
      method: "POST",
      body: JSON.stringify({ displayName }),
    });
    if (!response.ok) {
      setError("Create failed");
      return;
    }
    setDisplayName("");
    await load();
  }

  async function handleArchive(id: string) {
    if (!window.confirm("Archive this computer? (Sprite is not destroyed in Phase 3C.1.)")) {
      return;
    }
    setError(null);
    const response = await cloudHostFetch(`/v1/computers/${id}`, { method: "DELETE" });
    if (!response.ok) {
      setError("Archive failed");
      return;
    }
    await load();
  }

  return (
    <div className="grid gap-8 lg:grid-cols-[1fr_280px]">
      <section className="border border-brand-dark/15 bg-white p-5">
        <h2 className="text-sm font-medium uppercase tracking-[0.15em]">Computers</h2>
        <p className="mt-2 text-xs text-brand-dark/55">
          Fly Sprites provision lazily on first run. Provider resource IDs are assigned server-side.
        </p>
        <ul className="mt-4 space-y-3">
          {computers.map((computer) => (
            <li key={computer.id} className="flex items-start justify-between gap-3 border border-brand-dark/10 p-3">
              <div>
                <p className="font-medium">{computer.displayName}</p>
                <p className="mt-1 text-xs text-brand-dark/55">
                  {computer.state} · {computer.providerMetadata.provisioned ? "provisioned" : "pending"}
                </p>
                <p className="mt-1 font-mono text-[10px] text-brand-dark/40">{computer.id}</p>
              </div>
              <button
                type="button"
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

      <section className="border border-brand-dark/15 bg-white p-5">
        <h2 className="text-sm font-medium uppercase tracking-[0.15em]">New computer</h2>
        <form className="mt-4 space-y-3" onSubmit={(e) => void handleCreate(e)}>
          <div>
            <Label htmlFor="computer-name">Display name</Label>
            <Input
              id="computer-name"
              required
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
            />
          </div>
          {error ? (
            <p className="text-sm text-red-700" role="alert">
              {error}
            </p>
          ) : null}
          <Button type="submit">Create</Button>
        </form>
      </section>
    </div>
  );
}
