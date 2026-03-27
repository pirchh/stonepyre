CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ==============================
-- Portfolios
-- ==============================
CREATE TABLE IF NOT EXISTS portfolio.portfolios (
    portfolio_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    character_id UUID NOT NULL REFERENCES game.characters(character_id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    created_at   TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Optional: enforce 1 portfolio per character:
-- CREATE UNIQUE INDEX uq_portfolios_character ON portfolio.portfolios(character_id);

-- ==============================
-- Holdings (current positions)
-- ==============================
CREATE TABLE IF NOT EXISTS portfolio.holdings (
    portfolio_id UUID NOT NULL REFERENCES portfolio.portfolios(portfolio_id) ON DELETE CASCADE,
    company_id   BIGINT NOT NULL REFERENCES market.companies(company_id),
    shares       NUMERIC(18,6) NOT NULL DEFAULT 0,
    avg_cost     NUMERIC(18,6) NULL,
    PRIMARY KEY (portfolio_id, company_id)
);

-- ==============================
-- Transactions (audit log)
-- ==============================
CREATE TABLE IF NOT EXISTS portfolio.transactions (
    tx_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    portfolio_id UUID NOT NULL REFERENCES portfolio.portfolios(portfolio_id) ON DELETE CASCADE,
    company_id   BIGINT NOT NULL REFERENCES market.companies(company_id),
    ts           TIMESTAMP NOT NULL DEFAULT NOW(),
    side         TEXT NOT NULL, -- BUY or SELL
    shares       NUMERIC(18,6) NOT NULL,
    price        NUMERIC(18,6) NOT NULL,
    fee          NUMERIC(18,6) NOT NULL DEFAULT 0
);

ALTER TABLE portfolio.transactions
DROP CONSTRAINT IF EXISTS transactions_side_chk;

ALTER TABLE portfolio.transactions
ADD CONSTRAINT transactions_side_chk
CHECK (side IN ('BUY', 'SELL'));