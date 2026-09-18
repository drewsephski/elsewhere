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

const DEFAULT_DESKTOP_SHELL_ORIGINS = [
  "http://localhost:1420",
  "http://127.0.0.1:1420",
] as const;

function parseCommaSeparatedOrigins(raw: string | undefined): string[] {
  if (!raw?.trim()) {
    return [];
  }
  return raw
    .split(",")
    .map((entry) => entry.trim().replace(/\/$/, ""))
    .filter(Boolean);
}

/** Origins allowed to call Better Auth (includes the Tauri/Vite dev shell on :1420). */
export function authTrustedOrigins(options?: {
  appOrigin?: string;
  baseURL?: string;
  desktopOrigins?: string[];
}): string[] {
  const appOrigin = options?.appOrigin ?? publicAppOrigin();
  const baseURL = options?.baseURL ?? process.env.BETTER_AUTH_URL ?? appOrigin;
  const desktopOrigins =
    options?.desktopOrigins ??
    parseCommaSeparatedOrigins(process.env.ELSEWHERE_DESKTOP_TRUSTED_ORIGINS);
  const resolvedDesktop =
    desktopOrigins.length > 0 ? desktopOrigins : [...DEFAULT_DESKTOP_SHELL_ORIGINS];
  return [...new Set([appOrigin, baseURL, ...resolvedDesktop])];
}
