CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS game.characters (
    character_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id   UUID NOT NULL REFERENCES auth.accounts(account_id) ON DELETE CASCADE,

    name         TEXT NOT NULL,
    cash         NUMERIC(18,6) NOT NULL DEFAULT 0,

    created_at   TIMESTAMP NOT NULL DEFAULT NOW(),

    UNIQUE (account_id, name)
);