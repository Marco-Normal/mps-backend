CREATE TABLE IF NOT EXISTS imagens_produto (
  id         BIGSERIAL PRIMARY KEY,
  id_produto INTEGER NOT NULL REFERENCES produtos(id) ON DELETE CASCADE,
  path       TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_imagens_produto_produto ON imagens_produto(id_produto);
