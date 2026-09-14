/** Shared auth constants safe for client and server bundles. */
export const CLOUD_HOST_JWT_AUDIENCE =
  process.env.NEXT_PUBLIC_ELSEWHERE_JWT_AUDIENCE ?? "elsewhere-cloud-host";

export function cloudHostBaseUrl(): string {
  return (
    process.env.NEXT_PUBLIC_ELSEWHERE_CLOUD_HOST_URL?.replace(/\/$/, "") ??
    "http://127.0.0.1:8080"
  );
}

/** App origin for auth redirects (no trailing slash, no /api/auth path). */
export function publicAppOrigin(): string {
  const raw =
    process.env.NEXT_PUBLIC_BETTER_AUTH_URL ??
    process.env.BETTER_AUTH_URL ??
    "http://localhost:3000";
  return raw.replace(/\/api\/auth\/?$/, "").replace(/\/$/, "");
}
