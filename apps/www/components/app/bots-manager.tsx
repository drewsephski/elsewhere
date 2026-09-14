"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary, ComputerSummary } from "@/lib/api-types";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export function BotsManager() {
  const [bots, setBots] = useState<BotSummary[]>([]);
  const [computers, setComputers] = useState<ComputerSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [instructions, setInstructions] = useState("You are a helpful Luna assistant with access to the assigned computer.");
  const [computerId, setComputerId] = useState("");

  const load = useCallback(async () => {
    setError(null);
    try {
      const [botsRes, computersRes] = await Promise.all([
        cloudHostFetch("/v1/bots"),
        cloudHostFetch("/v1/computers"),
      ]);
      if (!botsRes.ok || !computersRes.ok) {
        throw new Error("Failed to load bots or computers");
      }
      setBots((await botsRes.json()) as BotSummary[]);
      const comps = (await computersRes.json()) as ComputerSummary[];
      setComputers(comps);
      setComputerId((current) => current || comps[0]?.id || "");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Load failed");
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleCreate(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    const response = await cloudHostFetch("/v1/bots", {
      method: "POST",
      body: JSON.stringify({
        name,
        instructions,
        computerId: computerId || undefined,
        enginePreference: "auto",
      }),
    });
    if (!response.ok) {
      const body = (await response.json().catch(() => ({}))) as { error?: string };
      setError(body.error ?? "Create failed");
      return;
    }
    setName("");
    await load();
  }

  return (
    <div className="grid gap-8 lg:grid-cols-[1fr_320px]">
      <section className="border border-brand-dark/15 bg-white p-5">
        <h2 className="text-sm font-medium uppercase tracking-[0.15em]">Your bots</h2>
        <ul className="mt-4 space-y-3">
          {bots.map((bot) => (
            <li key={bot.id} className="border border-brand-dark/10 p-3">
              <Link href={`/app/bots/${bot.id}`} className="font-medium underline-offset-4 hover:underline">
                {bot.name}
              </Link>
              <p className="mt-1 text-xs text-brand-dark/55">
                {bot.model} · {bot.enginePreference}
                {bot.computerId ? ` · computer ${bot.computerId.slice(0, 8)}…` : " · no computer"}
              </p>
            </li>
          ))}
          {bots.length === 0 ? (
            <li className="text-sm text-brand-dark/50">No bots yet.</li>
          ) : null}
        </ul>
      </section>

      <section className="border border-brand-dark/15 bg-white p-5">
        <h2 className="text-sm font-medium uppercase tracking-[0.15em]">New bot</h2>
        <form className="mt-4 space-y-3" onSubmit={(e) => void handleCreate(e)}>
          <div>
            <Label htmlFor="bot-name">Name</Label>
            <Input id="bot-name" required value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div>
            <Label htmlFor="bot-instructions">Instructions</Label>
            <textarea
              id="bot-instructions"
              className="mt-1 min-h-24 w-full border border-brand-dark/15 bg-white px-3 py-2 text-sm"
              value={instructions}
              onChange={(e) => setInstructions(e.target.value)}
            />
          </div>
          <div>
            <Label htmlFor="bot-computer">Computer</Label>
            <select
              id="bot-computer"
              className="mt-1 w-full border border-brand-dark/15 bg-white px-3 py-2 text-sm"
              value={computerId}
              onChange={(e) => setComputerId(e.target.value)}
            >
              {computers.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.displayName}
                </option>
              ))}
            </select>
          </div>
          {error ? (
            <p className="text-sm text-red-700" role="alert">
              {error}
            </p>
          ) : null}
          <Button type="submit" disabled={!computerId}>
            Create bot
          </Button>
        </form>
      </section>
    </div>
  );
}
