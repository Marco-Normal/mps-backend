-- Add up migration script here
CREATE TABLE IF NOT EXISTS pedidos (
       id BIGSERIAL,
       customer_id BIGINT NOT NULL, 
       stat status NOT NULL DEFAULT 'processando',
       created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
       updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
       ADD CONSTRAINT IF NOT EXISTS pedidos_fk PRIMARY KEY (id), 
);