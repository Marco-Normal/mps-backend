-- Add up migration script here
CREATE INDEX IF NOT EXISTS idx_order_items_order_id ON items_pedidos(id_order);