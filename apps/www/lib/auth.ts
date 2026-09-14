import { betterAuth } from "better-auth";
import { jwt } from "better-auth/plugins/jwt";
import { nextCookies } from "better-auth/next-js";
import { APIError, createAuthMiddleware } from "better-auth/api";
import { acceptsAlphaInvitation } from "@/lib/alpha-admission";
import { getAuthPool } from "@/lib/db";
import { CLOUD_HOST_JWT_AUDIENCE } from "@/lib/auth.shared";

const baseURL = process.env.BETTER_AUTH_URL ?? "http://localhost:3000";

const googleClientId = process.env.GOOGLE_CLIENT_ID?.trim();
const googleClientSecret = process.env.GOOGLE_CLIENT_SECRET?.trim();

function requireAlphaInvitation(headers?: Headers | null) {
  if (!acceptsAlphaInvitation(
    headers?.get("x-elsewhere-invite"),
    process.env.ELSEWHERE_ALPHA_INVITE_CODE,
    process.env.NEXT_PUBLIC_ELSEWHERE_ALPHA_INVITE_REQUIRED === "1",
  )) {
    throw new APIError("FORBIDDEN", {
      message: "A valid alpha invitation is required to create an account.",
    });
  }
}

export const auth = betterAuth({
  appName: "Elsewhere",
  baseURL,
  secret: process.env.BETTER_AUTH_SECRET,
  database: getAuthPool(),
  trustedOrigins: [baseURL],
  hooks: {
    before: createAuthMiddleware(async (context) => {
      // Reject uninvited requests before expensive password hashing.
      if (context.path === "/sign-up/email") requireAlphaInvitation(context.headers);
    }),
  },
  databaseHooks: {
    user: {
      create: {
        before: async (_user, context) => {
          requireAlphaInvitation(context?.headers);
        },
      },
    },
  },
  emailAndPassword: {
    enabled: true,
    autoSignIn: true,
  },
  ...(googleClientId && googleClientSecret
    ? {
        socialProviders: {
          google: {
            clientId: googleClientId,
            clientSecret: googleClientSecret,
          },
        },
      }
    : {}),
  plugins: [
    jwt({
      jwks: {
        keyPairConfig: { alg: "ES256" },
      },
      jwt: {
        issuer: baseURL,
        audience: CLOUD_HOST_JWT_AUDIENCE,
        expirationTime: "5m",
      },
    }),
    nextCookies(),
  ],
});

export type Session = typeof auth.$Infer.Session;
