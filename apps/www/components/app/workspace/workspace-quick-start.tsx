"use client";

import {
  rememberQuickStartDraft,
  type CreateBotOutcome,
} from "@/lib/bot-quick-start";
import { useRouter } from "next/navigation";
import { CreateBotForm } from "./create-bot-form";

interface WorkspaceQuickStartProps {
  onStarted?: () => void;
  onBusyChange?: (busy: boolean) => void;
}

export function WorkspaceQuickStart({
  onStarted,
  onBusyChange,
}: WorkspaceQuickStartProps) {
  const router = useRouter();

  function handleOutcome(outcome: CreateBotOutcome) {
    if (outcome.status === "started") {
      onStarted?.();
      router.push(`/app/bots/${outcome.botId}`);
      return;
    }
    if (outcome.status === "bot_ready_run_failed") {
      rememberQuickStartDraft(outcome.botId, outcome.task);
      onStarted?.();
      router.push(`/app/bots/${outcome.botId}`);
    }
  }

  return (
    <div className="flex flex-1 flex-col items-center justify-center overflow-y-auto px-6 py-8">
      <div className="flex w-full max-w-lg flex-col items-stretch gap-6">
        <section className="w-full rounded-2xl border border-border bg-card p-6 text-left">
          <h1 className="text-xl font-semibold tracking-tight text-foreground">
            Meet your first teammate
          </h1>
          <p className="mt-1.5 text-sm leading-6 text-muted-foreground">
            Name your Bot and tell it what to work on. Elsewhere sets up a computer and
            starts the job using your Codex allowance.
          </p>
          <CreateBotForm
            className="mt-5"
            requireTask
            submitLabel={undefined}
            onBusyChange={onBusyChange}
            onOutcome={handleOutcome}
          />
        </section>
      </div>
    </div>
  );
}
