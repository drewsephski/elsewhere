"use client";

import { ProviderStatusCard } from "@/components/app/provider-status-card";
import { Button } from "@/components/ui/button";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useProviderStatus } from "@/hooks/use-provider-status";
import {
  rememberQuickStartDraft,
  startFirstBotWork,
  type QuickStartSession,
} from "@/lib/bot-quick-start";
import { useRouter } from "next/navigation";
import { useRef, useState } from "react";

interface WorkspaceQuickStartProps {
  onStarted?: () => void;
  onBusyChange?: (busy: boolean) => void;
}

export function WorkspaceQuickStart({
  onStarted,
  onBusyChange,
}: WorkspaceQuickStartProps) {
  const router = useRouter();
  const {
    connected,
    checking,
    checkFailed,
    providerUnavailable,
    challenge,
  } = useProviderStatus();
  const [name, setName] = useState("");
  const [task, setTask] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<string | null>(null);
  const sessionRef = useRef<QuickStartSession>({});

  const needsCodex =
    !connected || Boolean(challenge) || checkFailed || providerUnavailable;
  const canSubmit =
    !busy &&
    connected &&
    !checking &&
    !challenge &&
    Boolean(name.trim()) &&
    Boolean(task.trim());

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (busy) {
      return;
    }
    if (!connected || challenge) {
      setError("Connect ChatGPT to start this Bot.");
      return;
    }
    setBusy(true);
    onBusyChange?.(true);
    setError(null);
    setProgress("Starting…");
    try {
      const outcome = await startFirstBotWork(
        { name, task },
        sessionRef.current,
      );
      if (outcome.status === "started") {
        onStarted?.();
        router.push(`/app/bots/${outcome.botId}`);
        return;
      }
      if (outcome.status === "bot_ready_run_failed") {
        rememberQuickStartDraft(outcome.botId, outcome.task);
        setError(outcome.error);
        onStarted?.();
        router.push(`/app/bots/${outcome.botId}`);
        return;
      }
      setError(outcome.error);
    } finally {
      setBusy(false);
      setProgress(null);
      onBusyChange?.(false);
    }
  }

  return (
    <div className="flex flex-1 flex-col items-center justify-center overflow-y-auto px-6 py-8">
      <div className="flex w-full max-w-lg flex-col items-stretch gap-6">
        {needsCodex ? (
          <ProviderStatusCard variant="featured" className="w-full max-w-none" />
        ) : null}
        <section className="w-full rounded-2xl border border-border bg-card p-6 text-left">
          <h1 className="text-xl font-semibold tracking-tight text-foreground">
            Meet your first teammate
          </h1>
          <p className="mt-1.5 text-sm leading-6 text-muted-foreground">
            Name your Bot and tell it what to work on. Elsewhere sets up a computer
            and starts the job using your Codex allowance.
          </p>
          <form className="mt-5" onSubmit={(event) => void handleSubmit(event)}>
            <FormFields>
              <FormItem>
                <Label htmlFor="quick-start-bot-name">Bot name</Label>
                <Input
                  id="quick-start-bot-name"
                  name="name"
                  required
                  maxLength={100}
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  placeholder="Scout"
                  disabled={busy}
                  autoComplete="off"
                />
              </FormItem>
              <FormItem>
                <Label htmlFor="quick-start-bot-task">
                  What should this Bot work on?
                </Label>
                <Textarea
                  id="quick-start-bot-task"
                  name="task"
                  required
                  maxLength={100000}
                  value={task}
                  onChange={(event) => setTask(event.target.value)}
                  placeholder="Research competitors and write a one-page brief"
                  disabled={busy}
                  className="min-h-36 text-base md:text-sm"
                />
              </FormItem>
              {error ? (
                <p className="text-sm text-destructive" role="alert">
                  {error}
                </p>
              ) : null}
              <Button
                type="submit"
                size="lg"
                className="w-full"
                disabled={!canSubmit}
              >
                {busy ? progress ?? "Starting…" : "Start working"}
              </Button>
              {!connected && !checking ? (
                <p className="text-center text-xs text-muted-foreground">
                  Connect ChatGPT above to start. Your Bot name and task stay here.
                </p>
              ) : (
                <p className="text-center text-xs text-muted-foreground">
                  Uses your Codex allowance. Elsewhere never switches to paid API usage
                  automatically.
                </p>
              )}
            </FormFields>
          </form>
        </section>
      </div>
    </div>
  );
}
