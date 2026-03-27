CREATE INDEX IF NOT EXISTS idx_companies_status
ON market.companies(status);

CREATE INDEX IF NOT EXISTS idx_companies_industry_status
ON market.companies(industry_id, status);

CREATE INDEX IF NOT EXISTS idx_prices_ts_desc
ON market.prices(ts DESC);

CREATE INDEX IF NOT EXISTS idx_prices_company_ts_desc
ON market.prices(company_id, ts DESC);

CREATE INDEX IF NOT EXISTS idx_events_company_ts
ON market.events(company_id, ts DESC);