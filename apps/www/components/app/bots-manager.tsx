"use client";

import type { CreateBotOutcome } from "@/lib/bot-quick-start";
import { botConversationHref, rememberOnboardingOffer } from "@/lib/bot-onboarding";
import { useRouter } from "next/navigation";
import { CreateBotForm } from "@/components/app/workspace/create-bot-form";
import { WorkspaceOverview } from "./workspace-overview";

export function BotsManager() {
  const router = useRouter();

  function handleOutcome(outcome: CreateBotOutcome) {
    if (outcome.status === "created") {
      rememberOnboardingOffer(outcome.botId);
      router.push(botConversationHref(outcome.botId, { setup: true }));
      return;
    }
    if (outcome.status === "started" || outcome.status === "bot_ready_run_failed") {
      router.push(botConversationHref(outcome.botId));
    }
  }

  return (
    <div className="grid items-start gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,28rem)]">
      <WorkspaceOverview compact />
      <section className="surface-card p-6">
        <h2 className="text-lg font-semibold tracking-tight">Meet your next teammate</h2>
        <p className="mt-2 text-sm leading-6 text-muted-foreground">
          Name them, describe what they own, and optionally hand off a first task. Elsewhere
          picks a computer and model unless you open Advanced.
        </p>
        <CreateBotForm className="mt-5" onOutcome={handleOutcome} />
      </section>
    </div>
  );
}
