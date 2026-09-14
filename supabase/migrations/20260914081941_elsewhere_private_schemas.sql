-- Supabase bootstrap only. Application tables remain managed by SQLx and Better Auth.
-- No application credentials or user data belong in migrations.
CREATE ROLE elsewhere_runner NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS CONNECTION LIMIT 12;
CREATE ROLE elsewhere_web NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS CONNECTION LIMIT 6;
GRANT elsewhere_runner, elsewhere_web TO postgres;

CREATE SCHEMA elsewhere_control AUTHORIZATION elsewhere_runner;
CREATE SCHEMA elsewhere_auth AUTHORIZATION elsewhere_web;
REVOKE ALL ON SCHEMA elsewhere_control, elsewhere_auth FROM PUBLIC, anon, authenticated, service_role;

ALTER ROLE elsewhere_runner SET search_path = elsewhere_control;
ALTER ROLE elsewhere_web SET search_path = elsewhere_auth;
ALTER ROLE elsewhere_runner SET idle_in_transaction_session_timeout = '30s';
ALTER ROLE elsewhere_web SET idle_in_transaction_session_timeout = '30s';

-- Future functions must not inherit PostgreSQL's default PUBLIC execute grant.
ALTER DEFAULT PRIVILEGES FOR ROLE elsewhere_runner REVOKE EXECUTE ON FUNCTIONS FROM PUBLIC;
ALTER DEFAULT PRIVILEGES FOR ROLE elsewhere_web REVOKE EXECUTE ON FUNCTIONS FROM PUBLIC;
ALTER DEFAULT PRIVILEGES FOR ROLE elsewhere_runner IN SCHEMA elsewhere_control REVOKE ALL ON TABLES FROM PUBLIC, anon, authenticated, service_role;
ALTER DEFAULT PRIVILEGES FOR ROLE elsewhere_runner IN SCHEMA elsewhere_control REVOKE ALL ON SEQUENCES FROM PUBLIC, anon, authenticated, service_role;
ALTER DEFAULT PRIVILEGES FOR ROLE elsewhere_web IN SCHEMA elsewhere_auth REVOKE ALL ON TABLES FROM PUBLIC, anon, authenticated, service_role;
ALTER DEFAULT PRIVILEGES FOR ROLE elsewhere_web IN SCHEMA elsewhere_auth REVOKE ALL ON SEQUENCES FROM PUBLIC, anon, authenticated, service_role;

-- Never include these private schemas in PostgREST's exposed schemas.
-- Login passwords are provisioned separately through a secret-safe operational path.
