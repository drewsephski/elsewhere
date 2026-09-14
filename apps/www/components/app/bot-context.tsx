"use client";
import { useCallback, useEffect, useState } from "react";
import { cloudHostFetch } from "@/lib/cloud-api";
import { Button } from "@/components/ui/button";
import { FormFields } from "@/components/ui/form-item";

type Context = { content: string; revision: number };

export function BotContext({
  botId,
  embedded = false,
}: {
  botId: string;
  embedded?: boolean;
}) {
  const [saved, setSaved] = useState<Context | null>(null);
  const [content, setContent] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  const load = useCallback(async () => {
    setBusy(true); setError(null);
    try {
      const response = await cloudHostFetch(`/v1/bots/${botId}/context`);
      if (!response.ok) throw new Error("Could not load saved context");
      const body: Context = await response.json(); setSaved(body); setContent(body.content);
    } catch (err) { setError(err instanceof Error ? err.message : "Could not load context"); }
    finally { setBusy(false); }
  }, [botId]);
  useEffect(() => { void load(); }, [load]);
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (!saved || busy) return;
    setBusy(true); setError(null); setNotice("");
    try {
      const response = await cloudHostFetch(`/v1/bots/${botId}/context`, { method: "PUT", body: JSON.stringify({ content, revision: saved.revision }) });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not save context");
      setSaved(body); setContent(body.content); setNotice("Saved. New assignments and routines will use this context.");
    } catch (err) { setError(err instanceof Error ? err.message : "Could not save context"); }
    finally { setBusy(false); }
  }
  const form = (
    <form className={embedded ? "px-1" : "mt-4"} onSubmit={(event) => void save(event)}>
      <FormFields>
      {!embedded ? (
        <>
          <h2 className="text-base font-semibold">What your bot should remember</h2>
          <p className="mt-2 text-sm text-muted-foreground">
            Keep project facts, preferences, and recurring instructions here. You control this
            memory. Leave passwords and sensitive credentials out.
          </p>
        </>
      ) : (
        <p className="text-xs leading-relaxed text-muted-foreground">
          Saved notes for future assignments. Skip passwords and secrets.
        </p>
      )}
      <label htmlFor="bot-context" className="sr-only">
        Saved context
      </label>
      <textarea
        id="bot-context"
        value={content}
        onChange={(event) => {
          setContent(event.target.value);
          setNotice("");
        }}
        disabled={busy || !saved}
        maxLength={16000}
        placeholder="Audience, tone, project facts…"
        className={
          embedded
            ? "min-h-24 w-full rounded-xl border border-border/80 bg-white/80 p-2.5 text-xs leading-5"
            : "min-h-32 w-full rounded-xl border border-border bg-background p-3 text-xs leading-5"
        }
      />
      <div className="flex flex-wrap items-center justify-between gap-2">
        {!embedded ? (
          <p className="text-xs text-muted-foreground">Applies to future work. Approvals still apply.</p>
        ) : null}
        <div className="ml-auto flex gap-2">
          {error ? (
            <Button type="button" variant="outline" size="sm" disabled={busy} onClick={() => void load()}>
              Reload
            </Button>
          ) : null}
          <Button type="submit" size={embedded ? "sm" : "default"} disabled={busy || !saved || content === saved.content}>
            {busy ? (saved ? "Saving…" : "Loading…") : "Save"}
          </Button>
        </div>
      </div>
      {error ? (
        <p role="alert" className="text-xs text-red-700">
          {error}
        </p>
      ) : null}
      {notice ? (
        <p role="status" className="text-xs text-muted-foreground">
          {notice}
        </p>
      ) : null}
      </FormFields>
    </form>
  );

  if (embedded) {
    return form;
  }

  return <section className="surface-card">{form}</section>;
}
