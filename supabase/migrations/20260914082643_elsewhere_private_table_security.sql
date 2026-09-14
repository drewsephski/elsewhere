-- Run after SQLx and Better Auth migrations. These are private server-owned
-- tables: application authorization still enforces Elsewhere user ownership.
-- Table owners retain access; all other roles receive no RLS policies.
DO $$
DECLARE
  target record;
BEGIN
  FOR target IN
    SELECT schemaname, tablename FROM pg_tables
    WHERE schemaname IN ('elsewhere_control', 'elsewhere_auth')
  LOOP
    EXECUTE format('ALTER TABLE %I.%I ENABLE ROW LEVEL SECURITY', target.schemaname, target.tablename);
    EXECUTE format('REVOKE ALL ON TABLE %I.%I FROM PUBLIC, anon, authenticated, service_role', target.schemaname, target.tablename);
  END LOOP;
END;
$$;
