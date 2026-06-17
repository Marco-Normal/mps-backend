#!/bin/bash
set -e

# This script runs inside any PostgreSQL container when it's first initialized.
# It runs as the superuser ($POSTGRES_USER).
# Reads generic env vars set by docker-compose per service.

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    -- Create migration user (DDL: can create tables, sequences, indexes, run migrations)
    DO \$\$
    BEGIN
        IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = '${MIGRATION_USER}') THEN
            CREATE USER ${MIGRATION_USER} WITH PASSWORD '${MIGRATION_PASSWORD}';
        END IF;
    END
    \$\$;
    GRANT CONNECT ON DATABASE ${POSTGRES_DB} TO ${MIGRATION_USER};
    GRANT CREATE ON DATABASE ${POSTGRES_DB} TO ${MIGRATION_USER};
    GRANT ALL PRIVILEGES ON SCHEMA public TO ${MIGRATION_USER};

    -- Create application user (DML only: no DDL, used by API services)
    DO \$\$
    BEGIN
        IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = '${APP_USER}') THEN
            CREATE USER ${APP_USER} WITH PASSWORD '${APP_PASSWORD}';
        END IF;
    END
    \$\$;
    GRANT CONNECT ON DATABASE ${POSTGRES_DB} TO ${APP_USER};

    -- Default privileges for future tables created by admin (belt-and-suspenders)
    ALTER DEFAULT PRIVILEGES IN SCHEMA public
        GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO ${APP_USER};
    ALTER DEFAULT PRIVILEGES IN SCHEMA public
        GRANT USAGE, SELECT ON SEQUENCES TO ${APP_USER};
EOSQL
