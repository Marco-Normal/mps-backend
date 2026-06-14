ALTER TABLE produtos
  ADD COLUMN descricao TEXT,
  ADD COLUMN estoque INTEGER NOT NULL DEFAULT 0 CHECK (estoque >= 0);
