"use client";

import { ApprovalCard } from "@/components/app/approval-card";
import { HumanInterventionBanner } from "@/components/app/human-intervention-banner";
import { RunActivityLine } from "@/components/app/run-activity-line";
import { SubagentCard } from "@/components/app/subagent-card";
import { UserQuestionCard } from "@/components/app/user-question-card";
import type { RunActivityItem } from "@/contexts/active-run-context";
import type { PendingHumanIntervention } from "@/lib/human-intervention";

interface RunConversationTimelineProps {
  items: RunActivityItem[];
  botName?: string;
  pendingHumanIntervention?: PendingHumanIntervention | null;
}

export function RunConversationTimeline({
  items,
  botName,
  pendingHumanIntervention,
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
