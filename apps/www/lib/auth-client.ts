"use client";

import { createAuthClient } from "better-auth/react";
import { jwtClient } from "better-auth/client/plugins";

export const authClient = createAuthClient({
  baseURL: process.env.NEXT_PUBLIC_BETTER_AUTH_URL,
  plugins: [jwtClient()],
});

export async function fetchCloudHostJwt(signal?: AbortSignal): Promise<string> {
  const base = process.env.NEXT_PUBLIC_BETTER_AUTH_URL ?? "";
  const response = await fetch(`${base}/api/auth/token`, {
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
