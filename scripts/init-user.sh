#!/bin/bash
set -e

# This script runs inside the PostgreSQL container when it's first initialized.
# It runs as the superuser ($POSTGRES_USER).

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    -- Create migration user (can create tables, sequences, indexes)
    CREATE USER ${PRODUTOS_MIGRATION_USER} WITH PASSWORD '${PRODUTOS_MIGRATION_PASSWORD}';
    GRANT CONNECT ON DATABASE ${POSTGRES_DB} TO ${PRODUTOS_MIGRATION_USER};
    GRANT CREATE ON DATABASE ${POSTGRES_DB} TO ${PRODUTOS_MIGRATION_USER};
    -- Grant all privileges on the public schema
    GRANT ALL PRIVILEGES ON SCHEMA public TO ${PRODUTOS_MIGRATION_USER};

    -- Create application user (only DML, no DDL)
    CREATE USER ${APP_USER} WITH PASSWORD '${APP_PASSWORD}';
    GRANT CONNECT ON DATABASE ${POSTGRES_DB} TO ${APP_USER};
    -- For future tables
    ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO ${APP_USER};
    ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO ${APP_USER};
EOSQL