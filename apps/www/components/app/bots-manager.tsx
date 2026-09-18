"use client";
import { cloudHostFetch } from "@/lib/cloud-api";
import type { ComputerSummary } from "@/lib/api-types";
import Link from "next/link";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { ComputerSelect } from "@/components/app/computer-select";
import { BotModelSelect } from "@/components/app/bot-model-select";
import { Button } from "@/components/ui/button";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { WorkspaceOverview } from "./workspace-overview";
import { DEFAULT_BOT_INSTRUCTIONS } from "@/lib/bot-quick-start";
import { DEFAULT_BOT_MODEL_ID } from "@/lib/bot-models";

export function BotsManager() {
  const router = useRouter();
  const [computers, setComputers] = useState<ComputerSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [instructions, setInstructions] = useState(DEFAULT_BOT_INSTRUCTIONS);
  const [computerId, setComputerId] = useState("");
  const [model, setModel] = useState(DEFAULT_BOT_MODEL_ID);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch("/v1/computers", { signal: controller.signal }).then(async response => {
      if (!response.ok) throw new Error("Could not load computers");
      const items: ComputerSummary[] = await response.json(); setComputers(items); setComputerId(items[0]?.id ?? "");
    }).catch(err => { if (!controller.signal.aborted) setError(err instanceof Error ? err.message : "Could not load computers"); })
      .finally(() => { if (!controller.signal.aborted) setLoading(false); });
    return () => controller.abort();
  }, []);
  async function create(event: React.FormEvent) {
    event.preventDefault(); if (busy) return; setBusy(true); setError(null);
    try {
      const response = await cloudHostFetch("/v1/bots", { method: "POST", body: JSON.stringify({ name, instructions, computerId, model, enginePreference: "codex" }) });
      const body = await response.json(); if (!response.ok) throw new Error(body.error ?? "Could not create bot");
      router.push(`/app/bots/${body.id}`);
    } catch (err) { setError(err instanceof Error ? err.message : "Could not create bot"); }
    finally { setBusy(false); }
  }
  return <div className="grid items-start gap-6 lg:grid-cols-[minmax(0,1fr)_320px]">
    <WorkspaceOverview compact />
    <section className="surface-card"><h2 className="text-lg font-semibold">Meet your next teammate</h2><p className="mt-2 text-sm text-muted-foreground">Give it a name, a role, a model, and a computer to work on.</p>
      <form className="mt-5" onSubmit={event => void create(event)}>
        <FormFields>
        <FormItem><Label htmlFor="bot-name">Name</Label><Input id="bot-name" placeholder="Scout" required maxLength={100} value={name} onChange={event => setName(event.target.value)} /></FormItem>
        <FormItem><Label htmlFor="bot-instructions">Role and instructions</Label><Textarea id="bot-instructions" className="min-h-36" maxLength={16000} value={instructions} onChange={event => setInstructions(event.target.value)} /></FormItem>
        <BotModelSelect id="bot-model" value={model} onValueChange={setModel} disabled={busy} />
        <ComputerSelect id="bot-computer" value={computerId} onValueChange={setComputerId} computers={computers} loading={loading} disabled={loading} />
        {!loading && !computers.length ? <p className="text-sm text-muted-foreground"><Link href="/app/computers" className="underline">Create a computer</Link> first. Its files will persist between assignments.</p> : null}
        {error ? <p role="alert" className="text-sm text-destructive">{error}</p> : null}
        <Button type="submit" disabled={busy || !computerId || !name.trim()}>{busy ? "Creating…" : "Create bot"}</Button>
        <p className="text-xs text-muted-foreground">Powered by your ChatGPT connection.</p>
        </FormFields>
      </form>
    </section>
  </div>;
}
