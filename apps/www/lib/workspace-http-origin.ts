import { publicAppOrigin } from "@/lib/auth.shared";
import { isTauriRuntime } from "@/lib/tauri-runtime";

const AUTH_PATH_SUFFIX = /\/api\/auth\/?$/;

/**
 * Hosted www origin used for auth cookies and the `/api/cloud` BFF when the UI is
 * bundled inside the Tauri shell (production). Browser and Vite dev keep same-origin
 * relative URLs (Next or the dev proxy on :1420).
 */
export function hostedWorkspaceOrigin(): string {
  if (!isTauriRuntime()) {
    return "";
  }
  const fromEnv =
    process.env.NEXT_PUBLIC_ELSEWHERE_WEB_ORIGIN ??
    process.env.NEXT_PUBLIC_BETTER_AUTH_URL;
  if (fromEnv?.trim()) {
    return fromEnv.trim().replace(AUTH_PATH_SUFFIX, "").replace(/\/$/, "");
  }
  if (import.meta.env.PROD) {
    return "https://elsewhere-alpha-web.fly.dev";
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
