/** Shared auth constants safe for client and server bundles. */
export const CLOUD_HOST_JWT_AUDIENCE =
  process.env.NEXT_PUBLIC_ELSEWHERE_JWT_AUDIENCE ?? "elsewhere-cloud-host";

export function cloudHostBaseUrl(): string {
  return (
    process.env.NEXT_PUBLIC_ELSEWHERE_CLOUD_HOST_URL?.replace(/\/$/, "") ??
    "http://127.0.0.1:8080"
  );
}
