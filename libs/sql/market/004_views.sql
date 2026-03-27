-- Active companies
CREATE OR REPLACE VIEW market.v_companies_active AS
SELECT *
FROM market.companies
WHERE status = 'ACTIVE'
  AND listed = TRUE;

-- Delisted / Bankrupt companies
CREATE OR REPLACE VIEW market.v_companies_delisted AS
SELECT *
FROM market.companies
WHERE status IN ('DELISTED', 'BANKRUPT')
   OR listed = FALSE;

-- Latest price per company
CREATE OR REPLACE VIEW market.v_latest_prices AS
SELECT DISTINCT ON (p.company_id)
    p.company_id,
    p.ts,
    p.price,
    p.volume
FROM market.prices p
ORDER BY p.company_id, p.ts DESC;

-- Market board (easy UI query)
CREATE OR REPLACE VIEW market.v_market_board AS
SELECT
    c.company_id,
    c.name,
    i.name AS industry,
    lp.price,
    lp.ts
FROM market.companies c
JOIN market.industries i ON i.industry_id = c.industry_id
LEFT JOIN market.v_latest_prices lp ON lp.company_id = c.company_id
WHERE c.status = 'ACTIVE'
  AND c.listed = TRUE;