import { createHash, timingSafeEqual } from "node:crypto";

/** Server-only signup gate; existing sessions do not need the invitation again. */
export function acceptsAlphaInvitation(
  supplied: string | null | undefined,
  configured: string | undefined,
  required: boolean,
): boolean {
  if (!configured) return !required;
  if (!supplied) return false;
  const digest = (value: string) => createHash("sha256").update(value).digest();
  return timingSafeEqual(digest(supplied), digest(configured));
}
