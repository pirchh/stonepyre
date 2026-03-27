-- ==============================
-- Sim Clock (single row)
-- 1 real hour = 1 sim day (60 sim minutes)
-- market open first 45 minutes, closed last 15
-- 360-day year, 90-day seasons
-- ==============================

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace
                   WHERE t.typname = 'season' AND n.nspname = 'market') THEN
        CREATE TYPE market.season AS ENUM ('SPRING', 'SUMMER', 'FALL', 'WINTER');
    END IF;
END$$;

CREATE TABLE IF NOT EXISTS market.clock (
    id                   INTEGER PRIMARY KEY CHECK (id = 1),

    -- real-time anchor (for debugging / visibility)
    started_at           TIMESTAMP NOT NULL DEFAULT NOW(),
    last_advance_at      TIMESTAMP NOT NULL DEFAULT NOW(),

    -- sim-time
    sim_day              INTEGER NOT NULL DEFAULT 0,         -- increments every "day"
    minute_of_day        INTEGER NOT NULL DEFAULT 0,         -- 0..(day_length_minutes-1)

    -- rules
    day_length_minutes   INTEGER NOT NULL DEFAULT 60,        -- 60 = 1 hour day
    market_close_minutes INTEGER NOT NULL DEFAULT 15,        -- last 15 minutes closed

    days_per_year        INTEGER NOT NULL DEFAULT 360,
    season_length_days   INTEGER NOT NULL DEFAULT 90,

    season               market.season NOT NULL DEFAULT 'SPRING',
    is_open              BOOLEAN NOT NULL DEFAULT TRUE
);

INSERT INTO market.clock (id)
VALUES (1)
ON CONFLICT (id) DO NOTHING;