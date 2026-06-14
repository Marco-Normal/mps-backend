CREATE TYPE order_status AS ENUM (
  'processando',
  'confirmado',
  'enviado',
  'entregue',
  'cancelado',
  'rejeitado'
);
