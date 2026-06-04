-- Add down migration script here
DROP INDEX IF EXISTS idx_produtos_nome_trgm;
DROP INDEX IF EXISTS idx_produtos_marca_trgm;