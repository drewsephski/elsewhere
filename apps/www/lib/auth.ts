import { betterAuth } from "better-auth";
import { jwt } from "better-auth/plugins/jwt";
import { nextCookies } from "better-auth/next-js";
import { getAuthPool } from "@/lib/db";
import { CLOUD_HOST_JWT_AUDIENCE } from "@/lib/auth.shared";

const baseURL = process.env.BETTER_AUTH_URL ?? "http://localhost:3000";

const googleClientId = process.env.GOOGLE_CLIENT_ID?.trim();
const googleClientSecret = process.env.GOOGLE_CLIENT_SECRET?.trim();

export const auth = betterAuth({
  appName: "Elsewhere",
  baseURL,
  secret: process.env.BETTER_AUTH_SECRET,
  database: getAuthPool(),
  trustedOrigins: [baseURL],
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
