import { cloudHostBaseUrl } from "@/lib/auth.shared";

/** Server-side upstream for the cloud-host BFF (prefer Fly 6PN when configured). */
export function cloudHostUpstreamBaseUrl(): string {
  const internal = process.env.ELSEWHERE_CLOUD_HOST_INTERNAL_URL?.trim();
  if (internal) {
    return internal.replace(/\/$/, "");
  }
  return cloudHostBaseUrl();
}
