# Supabase alpha control plane

Provisioned September 14, 2026 with explicit approval for Supabase Free in **drew's projects**. No other existing project was changed.

| Setting | Value |
| --- | --- |
| Project | Elsewhere Alpha |
| Reference | `edbfhcveqxtxnfybhmej` |
| Organization | `vercel_icfg_tzAfejCKM8R5lQMIn21z7SB7` |
| Plan / region | Free / `us-east-2` |
| Database | PostgreSQL 17.6, `postgres` |
| Direct endpoint | `db.edbfhcveqxtxnfybhmej.supabase.co:5432` (IPv6) |
| Runner role / schema | `elsewhere_runner` / `elsewhere_control` |
| Web role / schema | `elsewhere_web` / `elsewhere_auth` |
| Dashboard | [Elsewhere Alpha](https://supabase.com/dashboard/project/edbfhcveqxtxnfybhmej) |

Supabase supplies PostgreSQL only. Keep Better Auth, existing ownership checks, SQLx queues, and Codex-managed pairing. No Supabase client SDK, public client key, service-role key, Auth migration, Storage integration, or API fallback is needed.

## Connections and secrets

Use separate server-only URLs, `sslmode=verify-full`, and `sslrootcert` pointing at the checked-in **public** CA certificate in `infra/certs/supabase-prod-ca-2021.crt`. Set `BETTER_AUTH_DATABASE_SCHEMA=elsewhere_auth`. The default remains `auth` for existing local databases. Do not use Supabase's reserved `auth` schema for Better Auth.

The official certificate was downloaded through the database dashboard from [Supabase's certificate endpoint](https://supabase-downloads.s3-ap-southeast-1.amazonaws.com/prod/ssl/prod-ca-2021.crt). SHA-256 fingerprint: `80:70:25:AD:50:D4:ED:21:9D:2C:9C:7D:29:9C:00:4F:82:4E:B0:0C:F7:F6:5A:FE:F6:07:D0:7B:72:E6:CA:FA`; expires April 26, 2031. Include it in both deployment images and change the URL's certificate path to the absolute path inside each image. Do not disable verification to work around a missing CA.

New per-service database passwords and application secrets are staged in ignored `.env.hosted` with mode 0600. That file is local deployment staging, not a backup or deployment. It contains no Codex OAuth credentials or paid OpenAI API key. Import only each service's required values into Fly secrets via stdin. Keep the operator's local `.env` and local accounts unchanged. The auth URL currently used for migration is not a hosted issuer.

Use the direct connection for the runner's session advisory lock. Do not use transaction pooling. Web connections are capped at five; role limits are six web and twelve runner. The application roles are schema owners, not superusers, and have neither database/role creation nor BYPASSRLS privileges. They retain migration privileges within their own schema for the simple alpha deployment.

## Rebuild and migration order

1. Create the project on the explicitly selected Free organization. Apply `supabase/migrations/20260914081941_elsewhere_private_schemas.sql` through the authenticated Supabase migration tool.
2. Generate new passwords securely and enable LOGIN on the two roles through an operational SQL connection. Never commit passwords into migration history. Store replacement secrets in the deployment secret manager.
3. Enable database SSL enforcement in the dashboard. Disable Data API under Integrations → Data API → Overview. Both settings were saved and verified for this project.
4. Start the existing cloud-host using the runner URL; it acquires leadership and applies `services/cloud-host/migrations` in its private schema. Run `pnpm --filter @elsewhere/www auth:migrate --yes` with the web URL and schema. This uses the installed, pinned Better Auth 1.4.21 CLI.
5. Apply `supabase/migrations/20260914082643_elsewhere_private_table_security.sql` **after** both application migrations. It enables RLS and removes client grants on their tables. It is safe to re-run its SQL after future application migrations so newly added tables receive the same protection. Do not rerun the role bootstrap on an existing project.
6. Run the isolation checks below and Supabase security/performance advisors. Confirm no application tables leaked into `public` or reserved schemas.

Supabase migrations own project bootstrap/security only. SQLx and Better Auth retain their own table migration histories. Do not run a blanket `supabase db push` that skips this application-migration ordering or duplicate application DDL into Supabase history.

The remote MCP migration history assigns server timestamps: `20260914082040` for `elsewhere_private_schemas` and `20260914082712` for `elsewhere_private_table_security`. The corresponding local files retain their CLI-generated creation timestamps. Match these by name/content; reconcile history explicitly before any future CLI-managed remote migration workflow so bootstrap is not reapplied.

The authenticated Supabase plugin points at the correct organization. The installed local Supabase CLI currently lists a different account's projects; do not use it for remote operations until it is authenticated to this organization. Local `supabase migration new` was used only to generate filenames.

## Verified evidence

- All nine SQLx migrations and the pinned Better Auth migration completed against hosted PostgreSQL. There are 13 control-plane tables (including migration history) and five Better Auth tables.
- After applying RLS, the actual runner reached readiness and exited cleanly on SIGTERM in two successive local process starts against hosted Postgres. The actual web pool and auth schema initialization passed with the restricted role. Initialization now checks whether the schema exists before attempting creation, because PostgreSQL requires database CREATE even for `CREATE SCHEMA IF NOT EXISTS`.
- Both service roles connected over verified TLS, selected the correct schema, wrote/read a test row inside a transaction, and rolled it back. No acceptance fixture was retained.
- Each role was denied access to the other application's schema. Unencrypted direct connections were rejected. Supabase `anon`, `authenticated`, and `service_role` have no USAGE on either private schema.
- RLS is enabled on all 18 application tables. No client policies exist: owners can operate the server tables, while other roles default to denied access. Per-user isolation remains the responsibility of Elsewhere's tested owner-scoped APIs; these database checks prove service-role separation only.
- Security advisor: no warning/error findings; 18 informational [RLS without policies notices](https://supabase.com/docs/guides/database/database-linter?lint=0008_rls_enabled_no_policy) reflect this intentional owner-only access model. Do not add permissive policies to silence them.
- Performance advisor: four informational [unindexed foreign keys](https://supabase.com/docs/guides/database/database-linter?lint=0001_unindexed_foreign_keys) on `agent_runs.conversation_id` and `routines.bot_id`, `last_run_id`, `acknowledged_run_id`. No production load evidence justifies index changes yet. New-database [unused-index notices](https://supabase.com/docs/guides/database/database-linter?lint=0005_unused_index) are not grounds to remove queue/ownership indexes.

For isolation checks, connect as each actual application role, not `postgres`; inspect `current_schema()` and `pg_stat_ssl`, run a rolled-back write, and try reading the other schema. For RLS coverage, query `pg_tables` for both private schemas and require `bool_and(rowsecurity)`. Run these checks after each schema deployment. Never run the destructive integration test suite against this hosted database.

## Availability, storage, and recovery

The [Free plan](https://supabase.com/pricing) includes 500 MB database storage and 5 GB egress, may pause after one week of inactivity, and provides no automatic backups or PITR. Saved artifact bytes consume the database allowance. Monitor storage and treat paused/unavailable databases as a visible readiness failure. Do not automatically upgrade to Pro or delete user results when approaching quota.

Before inviting users, implement and prove daily encrypted logical backups of both application schemas with PostgreSQL 17 tooling, plus pre-upgrade copies outside the live volume. Keep database passwords, Better Auth signing/session secrets, bootstrap migrations, and opaque Codex volume snapshots recoverable through their respective secret/backup mechanisms. Stop web writes and drain the runner for coordinated recovery points. Restore into a separate database and verify ownership, migrations, accounts, queue recovery, and artifact bytes before switching URLs. No backup/restore drill has been performed yet.

This is hosted **database** evidence. The runner and web are not hosted yet. Real ChatGPT pairing, profile-volume persistence, laptop-off work/routines, Sprite persistence, downloaded artifacts, and recovery remain the acceptance gates in [the revised deployment proposal](HOSTED_ALPHA_PROPOSAL.md).
