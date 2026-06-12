-- Ephemeral intraday ticks (truncated each market close).
CREATE TABLE IF NOT EXISTS market.prices_ticks (
    company_id  BIGINT    NOT NULL REFERENCES market.companies(company_id) ON DELETE CASCADE,
    sim_day     INTEGER   NOT NULL,
    ts          TIMESTAMP NOT NULL,
    price       NUMERIC(18,6) NOT NULL,
    volume      BIGINT    NOT NULL DEFAULT 0,
    PRIMARY KEY (company_id, ts)
);

CREATE INDEX IF NOT EXISTS ix_market_prices_ticks_day
    ON market.prices_ticks (sim_day, company_id);

-- Daily OHLCV candles, one row per (company, sim_day).
CREATE TABLE IF NOT EXISTS market.candles_1h (
    company_id  BIGINT    NOT NULL REFERENCES market.companies(company_id) ON DELETE CASCADE,
    sim_day     INTEGER   NOT NULL,
    opened_at   TIMESTAMP NOT NULL,
    closed_at   TIMESTAMP NOT NULL,
    open        NUMERIC(18,6) NOT NULL,
    high        NUMERIC(18,6) NOT NULL,
    low         NUMERIC(18,6) NOT NULL,
    close       NUMERIC(18,6) NOT NULL,
    volume      BIGINT    NOT NULL DEFAULT 0,
    PRIMARY KEY (company_id, sim_day)
);

CREATE INDEX IF NOT EXISTS ix_market_candles_1h_day
    ON market.candles_1h (sim_day DESC);
