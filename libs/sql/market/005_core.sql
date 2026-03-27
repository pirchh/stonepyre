-- ==============================
-- Industries
-- ==============================

CREATE TABLE IF NOT EXISTS market.industries (
    industry_id      SERIAL PRIMARY KEY,
    code             TEXT UNIQUE NOT NULL,
    name             TEXT NOT NULL,
    cap              INTEGER NULL,
    weight           NUMERIC(10,4) NOT NULL DEFAULT 1.0,
    created_at       TIMESTAMP NOT NULL DEFAULT NOW()
);

-- ==============================
-- Companies
-- ==============================

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace
                   WHERE t.typname = 'company_status' AND n.nspname = 'market') THEN
        CREATE TYPE market.company_status AS ENUM ('ACTIVE', 'DELISTED', 'BANKRUPT');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace
                   WHERE t.typname = 'company_trend' AND n.nspname = 'market') THEN
        CREATE TYPE market.company_trend AS ENUM ('NORMAL', 'DECLINING', 'BOOMING');
    END IF;
END$$;

CREATE TABLE IF NOT EXISTS market.companies (
    company_id       BIGSERIAL PRIMARY KEY,
    name             TEXT NOT NULL,
    industry_id      INTEGER NOT NULL REFERENCES market.industries(industry_id),

    status           market.company_status NOT NULL DEFAULT 'ACTIVE',
    listed           BOOLEAN NOT NULL DEFAULT TRUE,

    created_at       TIMESTAMP NOT NULL DEFAULT NOW(),
    delisted_at      TIMESTAMP NULL,
    bankrupt_at      TIMESTAMP NULL,
    revived_at       TIMESTAMP NULL,

    revival_count    INTEGER NOT NULL DEFAULT 0,

    -- lifecycle / drift controls
    trend_state      market.company_trend NOT NULL DEFAULT 'NORMAL',
    trend_strength   NUMERIC(10,4) NOT NULL DEFAULT 0.0,
    trend_started_at TIMESTAMP NULL,

    base_volatility  NUMERIC(10,4) NOT NULL DEFAULT 0.05,
    quality_score    NUMERIC(10,4) NOT NULL DEFAULT 1.0
);

-- Optional but recommended: prevent duplicate company names (keeps UI clean)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE schemaname='market' AND indexname='ux_market_companies_name'
    ) THEN
        CREATE UNIQUE INDEX ux_market_companies_name ON market.companies (name);
    END IF;
END$$;

CREATE INDEX IF NOT EXISTS ix_market_companies_status_listed ON market.companies (status, listed);
CREATE INDEX IF NOT EXISTS ix_market_companies_industry ON market.companies (industry_id);

-- ==============================
-- Prices (time series)
-- ==============================

CREATE TABLE IF NOT EXISTS market.prices (
    company_id       BIGINT NOT NULL REFERENCES market.companies(company_id),
    ts               TIMESTAMP NOT NULL,
    price            NUMERIC(18,6) NOT NULL,
    volume           BIGINT NOT NULL DEFAULT 0,

    PRIMARY KEY (company_id, ts)
);

CREATE INDEX IF NOT EXISTS ix_market_prices_ts ON market.prices (ts DESC);

-- ==============================
-- Market Events
-- ==============================

CREATE TABLE IF NOT EXISTS market.events (
    event_id         BIGSERIAL PRIMARY KEY,
    company_id       BIGINT NOT NULL REFERENCES market.companies(company_id),
    ts               TIMESTAMP NOT NULL DEFAULT NOW(),
    event_type       TEXT NOT NULL,
    payload          JSONB NULL
);

CREATE INDEX IF NOT EXISTS ix_market_events_company_ts ON market.events (company_id, ts DESC);

-- ==============================
-- Market State (single row)
-- ==============================

CREATE TABLE IF NOT EXISTS market.market_state (
    id               INTEGER PRIMARY KEY CHECK (id = 1),
    tick             BIGINT NOT NULL DEFAULT 0,
    last_tick_at     TIMESTAMP NOT NULL DEFAULT NOW(),
    sim_seed         BIGINT NOT NULL,
    sim_version      TEXT NOT NULL
);

INSERT INTO market.market_state (id, sim_seed, sim_version)
VALUES (1, 42, 'v1')
ON CONFLICT (id) DO NOTHING;