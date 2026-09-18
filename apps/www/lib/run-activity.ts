import { activityText } from "./work-events";

export interface ActivityLine {
  headline: string;
  /** Raw tool or event name for progressive disclosure. */
  technical?: string;
}

function toolNameFromPayload(payload: Record<string, unknown>): string {
  const raw = String(payload.tool ?? payload.name ?? "").trim();
  if (!raw) {
    return "";
  }
  const segment = raw.includes("/") ? raw.split("/").pop() ?? raw : raw;
  return segment.replace(/^elsewhere[_-]/, "");
}

/**
 * User-facing activity line derived only from durable run events (no invented steps).
 */
export function activityLineFromEvent(
  event: string,
  payload: Record<string, unknown>,
): ActivityLine | null {
  const headline = activityText(event, payload);
  if (!headline) {
    return null;
  }
  const technical = toolNameFromPayload(payload);
  const showTechnical =
    Boolean(technical) &&
    !headline.toLowerCase().includes(technical.replace(/_/g, " ").toLowerCase());
  return {
    headline,
    technical: showTechnical ? technical : undefined,
  };
}
