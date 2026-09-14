import { Pool } from "pg";

const globalForPg = globalThis as unknown as { authPool?: Pool };

function databaseUrl(): string {
  const url = process.env.BETTER_AUTH_DATABASE_URL ?? process.env.DATABASE_URL;
  if (!url) {
    throw new Error("BETTER_AUTH_DATABASE_URL or DATABASE_URL is required for Elsewhere auth");
  }
  return url;
}

function authSchema(): string {
  const schema = process.env.BETTER_AUTH_DATABASE_SCHEMA ?? "auth";
  if (!/^[a-z_][a-z0-9_]{0,62}$/.test(schema)) {
    throw new Error("BETTER_AUTH_DATABASE_SCHEMA must be a simple PostgreSQL schema name");
  }
  return schema;
}

export function getAuthPool(): Pool {
  if (!globalForPg.authPool) {
    globalForPg.authPool = new Pool({
      connectionString: databaseUrl(),
      options: `-c search_path=${authSchema()}`,
      max: 5,
    });
  }
  return globalForPg.authPool;
}

let schemaReady: Promise<void> | null = null;

/** Ensures the dedicated Better Auth schema exists (tables come from `pnpm auth:migrate`). */
export async function ensureAuthSchema(): Promise<void> {
  if (!schemaReady) {
    const schema = authSchema();
    schemaReady = getAuthPool()
      // Even CREATE SCHEMA IF NOT EXISTS requires database-level CREATE.
      // Hosted roles only own their pre-provisioned schema.
      .query("SELECT 1 FROM pg_namespace WHERE nspname = $1", [schema])
      .then(async ({ rowCount }) => {
        if (rowCount === 0) {
          await getAuthPool().query(`CREATE SCHEMA IF NOT EXISTS "${schema}"`);
        }
      })
      .catch((error: unknown) => {
        schemaReady = null;
        throw error;
      });
  }
  await schemaReady;
}
