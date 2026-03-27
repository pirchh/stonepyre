-- ==============================
-- Industry seasonality multipliers
-- Controls drift/vol/volume/spawn/bankrupt bias by season
-- ==============================

CREATE TABLE IF NOT EXISTS market.industry_seasonality (
    industry_id      INTEGER NOT NULL REFERENCES market.industries(industry_id),
    season           market.season NOT NULL,

    drift_mult       NUMERIC(10,4) NOT NULL DEFAULT 1.0,
    vol_mult         NUMERIC(10,4) NOT NULL DEFAULT 1.0,
    volume_mult      NUMERIC(10,4) NOT NULL DEFAULT 1.0,
    bankrupt_mult    NUMERIC(10,4) NOT NULL DEFAULT 1.0,
    spawn_mult       NUMERIC(10,4) NOT NULL DEFAULT 1.0,

    PRIMARY KEY (industry_id, season)
);

-- Seed defaults for any existing industries (safe to run multiple times)
INSERT INTO market.industry_seasonality (industry_id, season, drift_mult, vol_mult, volume_mult, bankrupt_mult, spawn_mult)
SELECT i.industry_id, s.season, 1.0, 1.0, 1.0, 1.0, 1.0
FROM market.industries i
CROSS JOIN (VALUES ('SPRING'::market.season),
                   ('SUMMER'::market.season),
                   ('FALL'::market.season),
                   ('WINTER'::market.season)) s(season)
ON CONFLICT (industry_id, season) DO NOTHING;

-- Opinionated baselines (edit freely)
-- Maritime: winter dip
UPDATE market.industry_seasonality iss
SET drift_mult=0.90, vol_mult=1.10, volume_mult=0.80, bankrupt_mult=1.10, spawn_mult=0.80
FROM market.industries i
WHERE iss.industry_id=i.industry_id AND i.code='maritime' AND iss.season='WINTER';

UPDATE market.industry_seasonality iss
SET drift_mult=1.08, vol_mult=1.05, volume_mult=1.10, bankrupt_mult=0.95, spawn_mult=1.05
FROM market.industries i
WHERE iss.industry_id=i.industry_id AND i.code='maritime' AND iss.season='SUMMER';

-- Agriculture: fall harvest boom, winter strain
UPDATE market.industry_seasonality iss
SET drift_mult=1.10, vol_mult=1.05, volume_mult=1.15, bankrupt_mult=0.95, spawn_mult=1.10
FROM market.industries i
WHERE iss.industry_id=i.industry_id AND i.code='agriculture' AND iss.season='FALL';

UPDATE market.industry_seasonality iss
SET drift_mult=0.92, vol_mult=1.10, volume_mult=0.85, bankrupt_mult=1.10, spawn_mult=0.85
FROM market.industries i
WHERE iss.industry_id=i.industry_id AND i.code='agriculture' AND iss.season='WINTER';

-- Leatherwork: summer boom
UPDATE market.industry_seasonality iss
SET drift_mult=1.10, vol_mult=1.05, volume_mult=1.10, bankrupt_mult=0.95, spawn_mult=1.05
FROM market.industries i
WHERE iss.industry_id=i.industry_id AND i.code='leatherwork' AND iss.season='SUMMER';

-- Arcana: slightly “hotter” in winter (ritual season vibe)
UPDATE market.industry_seasonality iss
SET drift_mult=1.06, vol_mult=1.10, volume_mult=1.00, bankrupt_mult=0.98, spawn_mult=1.02
FROM market.industries i
WHERE iss.industry_id=i.industry_id AND i.code='arcana' AND iss.season='WINTER';

-- Extractives: mostly stable
UPDATE market.industry_seasonality iss
SET drift_mult=1.00, vol_mult=1.00, volume_mult=1.00, bankrupt_mult=1.00, spawn_mult=1.00
FROM market.industries i
WHERE iss.industry_id=i.industry_id AND i.code='extractives';