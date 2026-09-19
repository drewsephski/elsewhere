"use client";

import { ApprovalCard } from "@/components/app/approval-card";
import { ConnectorNeedCard } from "@/components/app/connector-need-card";
import { HumanInterventionBanner } from "@/components/app/human-intervention-banner";
import { RunActivityLine } from "@/components/app/run-activity-line";
import { SubagentCard } from "@/components/app/subagent-card";
import { UserQuestionCard } from "@/components/app/user-question-card";
import { RunToolActivity } from "@/components/app/workspace/chat-transcript";
import type { RunActivityItem } from "@/contexts/active-run-context";
import type { PendingHumanIntervention } from "@/lib/human-intervention";
import { botChatReturnTo } from "@/lib/connector-need";

interface RunConversationTimelineProps {
  items: RunActivityItem[];
  botName?: string;
  pendingHumanIntervention?: PendingHumanIntervention | null;
  returnToBotId?: string;
  returnToConversationId?: string | null;
}

export function RunConversationTimeline({
  items,
  botName,
  pendingHumanIntervention,
  returnToBotId,
  returnToConversationId,
}: RunConversationTimelineProps) {
  return (
    <>
      {pendingHumanIntervention ? (
        <HumanInterventionBanner pending={pendingHumanIntervention} />
      ) : null}
      {items.map((item) => {
        if (item.kind === "approval") {
          return (
            <ApprovalCard
              key={item.id}
              payload={item.approval}
              externalStatus={item.decision}
            />
          );
        }
        if (item.kind === "subagent") {
          return <SubagentCard key={item.id} activity={item.subagent} />;
        }
        if (item.kind === "question") {
          return (
            <UserQuestionCard key={item.id} question={item.question} botName={botName} />
          );
        }
        if (item.kind === "connector") {
          const returnTo = botChatReturnTo({
            botId: returnToBotId ?? item.need.botId,
            conversationId: returnToConversationId,
          });
          return <ConnectorNeedCard key={item.id} need={item.need} returnTo={returnTo} />;
        }
        if (item.kind === "tool") {
          return <RunToolActivity key={item.id} tool={item.tool} />;
        }
        return (
          <RunActivityLine
            key={item.id}
            line={{ headline: item.text, technical: item.technical }}
          />
        );
      })}
    </>
  );
}
