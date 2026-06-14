CREATE TABLE IF NOT EXISTS items_pedidos (
  id         BIGSERIAL PRIMARY KEY,
  id_order   BIGINT NOT NULL REFERENCES pedidos(id) ON DELETE CASCADE,
  id_product INTEGER NOT NULL,
  quantity   INTEGER NOT NULL CHECK (quantity > 0),
  unit_price DECIMAL(10,2) NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_items_pedidos_order ON items_pedidos(id_order);
