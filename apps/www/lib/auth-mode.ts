/** Auth form mode for the shared sign-in / sign-up UI. */
export type AuthFormMode = "sign-in" | "sign-up";

/** Resolve UI mode from the current pathname (`/sign-up` → sign-up). */
export function authModeFromPathname(pathname: string | null | undefined): AuthFormMode {
  if (!pathname) {
    return "sign-in";
  }
  const normalized =
    pathname.length > 1 && pathname.endsWith("/") ? pathname.slice(0, -1) : pathname;
  return normalized === "/sign-up" ? "sign-up" : "sign-in";
}

/** Path for the opposite auth mode (used by the sign-in ↔ sign-up toggle). */
export function authModeToggleHref(mode: AuthFormMode): string {
  return mode === "sign-in" ? "/sign-up" : "/sign-in";
}

/** Allow only the Mac pairing return path through sign-in `next`. */
export function safeAuthNextPath(raw: string | null | undefined): string | null {
  if (!raw) {
    return null;
  }
  if (!raw.startsWith("/pair/mac")) {
    return null;
  }
  if (raw.startsWith("//") || raw.includes("://") || raw.includes("\\")) {
    return null;
  }
  return raw;
}
