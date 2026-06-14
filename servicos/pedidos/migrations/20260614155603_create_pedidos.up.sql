CREATE TABLE IF NOT EXISTS pedidos (
  id          BIGSERIAL PRIMARY KEY,
  customer_id UUID NOT NULL,
  stat        order_status NOT NULL DEFAULT 'processando',
  created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pedidos_customer ON pedidos(customer_id);
CREATE INDEX idx_pedidos_stat     ON pedidos(stat);
