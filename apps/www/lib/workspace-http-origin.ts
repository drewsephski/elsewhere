import { publicAppOrigin } from "@/lib/auth.shared";
import { isTauriRuntime } from "@/lib/tauri-runtime";

const AUTH_PATH_SUFFIX = /\/api\/auth\/?$/;

const DEFAULT_HOSTED_ORIGIN = "https://elsewhere-alpha-web.fly.dev";

/** Known Elsewhere www origins (production + local Next dev for the Vite shell). */
export function isHostedElsewhereWebOrigin(origin: string): boolean {
  const normalized = origin.trim().replace(/\/$/, "");
  if (!normalized) {
    return false;
  }
  if (normalized === DEFAULT_HOSTED_ORIGIN) {
    return true;
  }
  if (
    normalized === "http://localhost:3000" ||
    normalized === "http://127.0.0.1:3000"
  ) {
    return true;
  }
  const fromEnv =
    process.env.NEXT_PUBLIC_ELSEWHERE_WEB_ORIGIN ??
    process.env.NEXT_PUBLIC_BETTER_AUTH_URL;
  if (fromEnv?.trim()) {
    const envOrigin = fromEnv.trim().replace(AUTH_PATH_SUFFIX, "").replace(/\/$/, "");
    return normalized === envOrigin;
  }
  return false;
}

/**
 * Cross-origin API base used only by the bundled Vite dev shell (localhost:1420).
 * Production desktop loads hosted Elsewhere first-party — returns empty string there.
 */
export function hostedWorkspaceOrigin(): string {
  if (!isTauriRuntime()) {
    return "";
  }
  if (typeof window !== "undefined") {
    if (isHostedElsewhereWebOrigin(window.location.origin)) {
      return "";
    }
  }
  const fromEnv =
    process.env.NEXT_PUBLIC_ELSEWHERE_WEB_ORIGIN ??
    process.env.NEXT_PUBLIC_BETTER_AUTH_URL;
  if (fromEnv?.trim()) {
    return fromEnv.trim().replace(AUTH_PATH_SUFFIX, "").replace(/\/$/, "");
  }
  if (import.meta.env?.DEV) {
    return DEFAULT_HOSTED_ORIGIN;
  }
  return "";
}

/** Absolute URL for workspace HTTP (auth + BFF). Empty string means same-origin relative. */
export function resolveWorkspaceHttpUrl(path: string): string {
  const normalized = path.startsWith("/") ? path : `/${path}`;
  const origin = hostedWorkspaceOrigin();
  if (!origin) {
    if (typeof window !== "undefined") {
      return normalized;
    }
    return `${publicAppOrigin()}${normalized}`;
  }
  return `${origin}${normalized}`;
}

export function cloudBffHttpUrl(apiPath: string): string {
  const path = apiPath.startsWith("/") ? apiPath : `/${apiPath}`;
  return resolveWorkspaceHttpUrl(`/api/cloud${path}`);
}

export function authHttpUrl(authPath: string): string {
  const path = authPath.startsWith("/") ? authPath : `/${authPath}`;
  const suffix = path.startsWith("/api/auth") ? path : `/api/auth${path}`;
  return resolveWorkspaceHttpUrl(suffix);
}
