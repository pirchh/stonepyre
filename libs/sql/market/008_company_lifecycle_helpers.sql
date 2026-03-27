-- ==============================
-- Helpers: simple view for “eligible revival”
-- ==============================

CREATE OR REPLACE VIEW market.v_revival_candidates AS
SELECT
    c.company_id,
    c.industry_id,
    c.bankrupt_at,
    c.revival_count
FROM market.companies c
WHERE c.status = 'BANKRUPT'
  AND c.bankrupt_at IS NOT NULL;