import { Pool } from "pg";

const globalForPg = globalThis as unknown as { authPool?: Pool };

function databaseUrl(): string {
  const url = process.env.BETTER_AUTH_DATABASE_URL ?? process.env.DATABASE_URL;
  if (!url) {
    throw new Error("BETTER_AUTH_DATABASE_URL or DATABASE_URL is required for Elsewhere auth");
  }
  return url;
}

export function getAuthPool(): Pool {
  if (!globalForPg.authPool) {
    globalForPg.authPool = new Pool({
      connectionString: databaseUrl(),
      options: "-c search_path=auth",
    });
  }
  return globalForPg.authPool;
}

let schemaReady: Promise<void> | null = null;

/** Ensures the dedicated Better Auth schema exists (tables come from `pnpm auth:migrate`). */
export async function ensureAuthSchema(): Promise<void> {
  if (!schemaReady) {
    schemaReady = getAuthPool().query("CREATE SCHEMA IF NOT EXISTS auth").then(() => undefined);
  }
  await schemaReady;
}
