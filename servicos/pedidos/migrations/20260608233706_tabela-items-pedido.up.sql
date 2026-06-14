-- Add up migration script here
CREATE TABLE IF NOT EXISTS items_pedidos (
       id BIGSERIAL NOT NULL,
       id_order BIGINT NOT NULL ON DELETE CASCADE,
       id_product INTEGER NOT NULL,
       quantity INTEGER NOT NULL CHECK (quantidade > 0),
       unit_price DECIMAL(10, 2) NOT NULL
       created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
       ADD CONSTRAINT items_pedido_pk PRIMARY KEY(id),
       ADD CONSTRAINT order_fk FOREIGN KEY (id_order) REFERENCES produtos(id)
);