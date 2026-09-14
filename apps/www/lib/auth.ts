import { betterAuth } from "better-auth";
import { jwt } from "better-auth/plugins/jwt";
import { nextCookies } from "better-auth/next-js";
import { APIError, createAuthMiddleware } from "better-auth/api";
import { acceptsAlphaInvitation } from "@/lib/alpha-admission";
import { getAuthPool } from "@/lib/db";
import { CLOUD_HOST_JWT_AUDIENCE, publicAppOrigin } from "@/lib/auth.shared";
import { sendAuthEmail } from "@/lib/send-auth-email";
import { siteConfig } from "@elsewhere/brand";

const appOrigin = publicAppOrigin();
const baseURL = process.env.BETTER_AUTH_URL ?? appOrigin;

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
  trustedOrigins: [appOrigin, baseURL],
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
    revokeSessionsOnPasswordReset: true,
    sendResetPassword: async ({ user, url }) => {
      void sendAuthEmail({
        to: user.email,
        subject: `Reset your ${siteConfig.productName} password`,
        text: `We received a request to reset your password.\n\nOpen this link to choose a new password (expires in one hour):\n${url}\n\nIf you did not request this, you can ignore this email.`,
        html: `<p>We received a request to reset your password.</p><p><a href="${url}">Choose a new password</a> (link expires in one hour).</p><p>If you did not request this, you can ignore this email.</p>`,
      });
    },
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
