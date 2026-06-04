-- Add up migration script here
CREATE INDEX IF NOT EXISTS idx_produtos_nome_trgm ON produtos USING gin (nome_norm gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_produtos_marca_trgm ON produtos USING gin (marca_norm gin_trgm_ops);