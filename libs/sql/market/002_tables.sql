-- ==============================
-- Industries
-- ==============================

CREATE TABLE IF NOT EXISTS market.industries (
    industry_id      SERIAL PRIMARY KEY,
    code             TEXT UNIQUE NOT NULL,
    name             TEXT NOT NULL,
    cap              INTEGER NULL,
    created_at       TIMESTAMP NOT NULL DEFAULT NOW()
);

-- ==============================
-- Companies
-- ==============================

CREATE TYPE market.company_status AS ENUM (
    'ACTIVE',
    'DELISTED',
    'BANKRUPT'
);

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

    base_volatility  NUMERIC(10,4) NOT NULL DEFAULT 0.05,
    quality_score    NUMERIC(10,4) NOT NULL DEFAULT 1.0
);

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