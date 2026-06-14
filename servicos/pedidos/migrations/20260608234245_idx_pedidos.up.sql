-- Add up migration script here
CREATE INDEX IF NOT EXISTS idx_order_customer_id on pedidos(customer_id);
CREATE INDEX IF NOT EXISTS idx_order_status on pedidos(stat);
