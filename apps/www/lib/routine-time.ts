export function formatRoutineNextRun(iso: string, timeZone: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso;
  }
  return new Intl.DateTimeFormat("en-US", {
    weekday: "long",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    timeZone,
    timeZoneName: "short",
  }).format(date);
}

export function formatRoutineTrigger(routine: {
  triggerMode?: string | null;
  scheduleLabel?: string | null;
}): string {
  if (routine.triggerMode === "webhook") {
    return "Webhook";
  }
  return routine.scheduleLabel || "Scheduled";
}

export function formatRoutineRunTrigger(kind: string): string {
  if (kind === "webhook") return "Webhook";
  if (kind === "scheduled") return "Scheduled";
  return "Manual/Test";
}

export function formatWebhookLastReceived(iso: string | null | undefined): string | null {
  if (!iso) return null;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return null;
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(date);
}
