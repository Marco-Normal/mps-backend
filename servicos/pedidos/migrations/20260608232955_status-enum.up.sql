-- Add up migration script here
CREATE TYPE IF NOT EXISTS status AS ENUM ("processando","confirmado", "enviado", "entregue", "cancelado", "rejeitado");