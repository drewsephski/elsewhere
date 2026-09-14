"use client";

import { createAuthClient } from "better-auth/react";
import { jwtClient } from "better-auth/client/plugins";

function authClientBaseUrl(): string | undefined {
  if (typeof window !== "undefined") {
    return window.location.origin;
  }
  return process.env.NEXT_PUBLIC_BETTER_AUTH_URL;
}

export const authClient = createAuthClient({
  baseURL: authClientBaseUrl(),
  plugins: [jwtClient()],
});

export async function fetchCloudHostJwt(signal?: AbortSignal): Promise<string> {
  const tokenPath =
    typeof window !== "undefined"
      ? "/api/auth/token"
      : `${process.env.NEXT_PUBLIC_BETTER_AUTH_URL ?? "http://localhost:3000"}/api/auth/token`;
  const response = await fetch(tokenPath, {
    method: "GET",
    cache: "no-store",
    signal,
    credentials: "include",
  });
  if (!response.ok) {
    throw new Error("Not signed in or JWT plugin unavailable");
  }
  const body = (await response.json()) as { token?: string };
  if (!body.token) {
    throw new Error("JWT response missing token");
  }
  return body.token;
}
