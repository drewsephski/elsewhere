"use client";

import { useEffect, useState } from "react";
import type { DelegationSummary } from "@/lib/api-types";
import { cloudHostFetch } from "@/lib/cloud-api";
import { DelegationCard } from "@/components/app/delegation-card";

interface RunDelegationListProps {
  runId: string;
  enabled: boolean;
}

export function RunDelegationList({ runId, enabled }: RunDelegationListProps) {
  const [delegations, setDelegations] = useState<DelegationSummary[]>([]);

  useEffect(() => {
    if (!enabled) {
      setDelegations([]);
      return;
    }
    const controller = new AbortController();
    cloudHostFetch(`/v1/runs/${runId}/delegations`, { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          return;
        }
        setDelegations(await response.json());
      })
      .catch(() => undefined);
    return () => controller.abort();
  }, [enabled, runId]);

  if (!delegations.length) {
    return null;
  }

  return (
    <div className="space-y-2">
      {delegations.map((delegation) => (
        <DelegationCard key={delegation.id} delegation={delegation} />
      ))}
    </div>
  );
}
