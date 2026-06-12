-- ==============================
-- Enum types
-- ==============================
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace
                   WHERE t.typname='lot_method' AND n.nspname='portfolio') THEN
        CREATE TYPE portfolio.lot_method AS ENUM ('FIFO','LIFO');
    END IF;
END $$;

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace
                   WHERE t.typname='order_status' AND n.nspname='portfolio') THEN
        CREATE TYPE portfolio.order_status AS ENUM ('OPEN','FILLED','CANCELLED','REJECTED');
    END IF;
END $$;

-- ==============================
-- portfolios: add lot_method column
-- ==============================
ALTER TABLE portfolio.portfolios
    ADD COLUMN IF NOT EXISTS lot_method portfolio.lot_method NOT NULL DEFAULT 'FIFO';

-- ==============================
-- holding_lots
-- ==============================
CREATE TABLE IF NOT EXISTS portfolio.holding_lots (
    lot_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    portfolio_id        UUID NOT NULL REFERENCES portfolio.portfolios(portfolio_id) ON DELETE CASCADE,
    company_id          BIGINT NOT NULL REFERENCES market.companies(company_id),
    opened_at           TIMESTAMP NOT NULL DEFAULT NOW(),
    source_tx_id        UUID NOT NULL,
    shares_opened       NUMERIC(18,6) NOT NULL,
    shares_remaining    NUMERIC(18,6) NOT NULL,
    cost_price          NUMERIC(18,6) NOT NULL,
    fee_allocated       NUMERIC(18,6) NOT NULL DEFAULT 0,
    buy_fee_total       NUMERIC(18,6) NOT NULL DEFAULT 0,
    buy_fee_remaining   NUMERIC(18,6) NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_lots_portfolio_company_opened
    ON portfolio.holding_lots (portfolio_id, company_id, opened_at);
CREATE INDEX IF NOT EXISTS idx_lots_portfolio_company_remaining
    ON portfolio.holding_lots (portfolio_id, company_id, shares_remaining);

-- FK added separately to avoid ordering issues with transactions table
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'holding_lots_source_tx_id_fkey') THEN
        ALTER TABLE portfolio.holding_lots
            ADD CONSTRAINT holding_lots_source_tx_id_fkey
            FOREIGN KEY (source_tx_id) REFERENCES portfolio.transactions(tx_id);
    END IF;
END $$;

-- ==============================
-- lot_consumptions
-- ==============================
CREATE TABLE IF NOT EXISTS portfolio.lot_consumptions (
    consumption_id  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sell_tx_id      UUID NOT NULL REFERENCES portfolio.transactions(tx_id),
    lot_id          UUID NOT NULL REFERENCES portfolio.holding_lots(lot_id),
    shares          NUMERIC(18,6) NOT NULL,
    buy_price       NUMERIC(18,6) NOT NULL,
    sell_price      NUMERIC(18,6) NOT NULL,
    buy_fee_alloc   NUMERIC(18,6) NOT NULL DEFAULT 0,
    sell_fee_alloc  NUMERIC(18,6) NOT NULL DEFAULT 0,
    realized_pnl    NUMERIC(18,6) NOT NULL,
    created_at      TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_lot_consumptions_lot
    ON portfolio.lot_consumptions (lot_id);
CREATE INDEX IF NOT EXISTS idx_lot_consumptions_sell_tx
    ON portfolio.lot_consumptions (sell_tx_id);

-- ==============================
-- orders
-- ==============================
CREATE TABLE IF NOT EXISTS portfolio.orders (
    order_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id      UUID NOT NULL REFERENCES auth.accounts(account_id),
    character_id    UUID NOT NULL REFERENCES game.characters(character_id),
    portfolio_id    UUID NOT NULL REFERENCES portfolio.portfolios(portfolio_id),
    company_id      BIGINT NOT NULL REFERENCES market.companies(company_id),
    side            TEXT NOT NULL CHECK (side IN ('BUY','SELL')),
    shares          NUMERIC(18,6) NOT NULL,
    fee             NUMERIC(18,6) NOT NULL DEFAULT 0,
    status          portfolio.order_status NOT NULL DEFAULT 'OPEN',
    reject_reason   TEXT NULL,
    placed_at       TIMESTAMP NOT NULL DEFAULT NOW(),
    placed_sim_day  INTEGER NULL,
    placed_minute   INTEGER NULL,
    filled_at       TIMESTAMP NULL,
    filled_sim_day  INTEGER NULL,
    filled_minute   INTEGER NULL,
    filled_price    NUMERIC(18,6) NULL,
    tx_id           UUID NULL REFERENCES portfolio.transactions(tx_id),
    cancelled_at    TIMESTAMP NULL
);

CREATE INDEX IF NOT EXISTS idx_orders_account_status_time
    ON portfolio.orders (account_id, status, placed_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_company_status_time
    ON portfolio.orders (company_id, status, placed_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_portfolio_status_time
    ON portfolio.orders (portfolio_id, status, placed_at DESC);

-- ==============================
-- v_positions view
-- ==============================
CREATE OR REPLACE VIEW portfolio.v_positions AS
WITH lot_rollup AS (
    SELECT l.portfolio_id,
           l.company_id,
           SUM(l.shares_remaining) AS shares,
           SUM((l.cost_price * l.shares_remaining) + l.buy_fee_remaining) AS cost_basis_remaining
    FROM portfolio.holding_lots l
    WHERE l.shares_remaining > 0
    GROUP BY l.portfolio_id, l.company_id
),
latest_prices AS (
    SELECT DISTINCT ON (p.company_id)
        p.company_id,
        p.price::NUMERIC(18,6) AS price,
        p.ts
    FROM market.prices p
    ORDER BY p.company_id, p.ts DESC
)
SELECT r.portfolio_id,
       r.company_id,
       r.shares,
       CASE WHEN r.shares > 0 THEN r.cost_basis_remaining / r.shares ELSE NULL END AS avg_cost_fee_aware,
       lp.price AS current_price,
       CASE WHEN lp.price IS NOT NULL THEN (lp.price * r.shares) - r.cost_basis_remaining ELSE NULL END AS unrealized_pnl
FROM lot_rollup r
LEFT JOIN latest_prices lp ON lp.company_id = r.company_id;

-- ==============================
-- execute_trade (lot-tracking version)
-- ==============================
CREATE OR REPLACE FUNCTION portfolio.execute_trade(
    p_session_token TEXT,
    p_character_id  UUID,
    p_portfolio_id  UUID,
    p_company_id    BIGINT,
    p_side          TEXT,
    p_shares        NUMERIC(18,6),
    p_price         NUMERIC(18,6),
    p_fee           NUMERIC(18,6) DEFAULT 0
)
RETURNS UUID AS $$
DECLARE
    v_account_id      UUID;
    v_tx_id           UUID := gen_random_uuid();
    v_cash            NUMERIC;
    v_lot_method      portfolio.lot_method;
    v_total_cost      NUMERIC;
    v_total_proceeds  NUMERIC;
    v_remaining       NUMERIC;
    v_sell_fee_per_sh NUMERIC;
    v_hold_shares     NUMERIC;
    rec               RECORD;
    v_take            NUMERIC;
    v_lot_sh_before   NUMERIC;
    v_buy_fee_per_sh  NUMERIC;
    v_buy_fee_take    NUMERIC;
    v_sell_fee_take   NUMERIC;
    v_realized        NUMERIC;
BEGIN
    IF p_shares IS NULL OR p_shares <= 0 THEN RAISE EXCEPTION 'shares must be > 0'; END IF;
    IF p_price  IS NULL OR p_price  <= 0 THEN RAISE EXCEPTION 'price must be > 0';  END IF;
    IF p_fee    IS NULL OR p_fee    <  0 THEN RAISE EXCEPTION 'fee must be >= 0';   END IF;
    IF p_side NOT IN ('BUY','SELL')      THEN RAISE EXCEPTION 'side must be BUY or SELL'; END IF;

    SELECT auth.verify_session(p_session_token) INTO v_account_id;
    IF v_account_id IS NULL THEN RAISE EXCEPTION 'unauthorized'; END IF;

    IF NOT EXISTS (SELECT 1 FROM game.characters c WHERE c.character_id = p_character_id AND c.account_id = v_account_id) THEN
        RAISE EXCEPTION 'forbidden: character not owned by account';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM portfolio.portfolios p WHERE p.portfolio_id = p_portfolio_id AND p.character_id = p_character_id) THEN
        RAISE EXCEPTION 'forbidden: portfolio not owned by character';
    END IF;

    SELECT p.lot_method INTO v_lot_method FROM portfolio.portfolios p WHERE p.portfolio_id = p_portfolio_id;

    SELECT cash INTO v_cash FROM game.characters WHERE character_id = p_character_id FOR UPDATE;

    IF p_side = 'BUY' THEN
        v_total_cost := (p_shares * p_price) + p_fee;
        IF v_cash < v_total_cost THEN
            RAISE EXCEPTION 'insufficient cash (need %, have %)', v_total_cost, v_cash;
        END IF;

        INSERT INTO portfolio.transactions (tx_id, portfolio_id, company_id, side, shares, price, fee)
        VALUES (v_tx_id, p_portfolio_id, p_company_id, 'BUY', p_shares, p_price, p_fee);

        INSERT INTO portfolio.holding_lots (
            lot_id, portfolio_id, company_id,
            opened_at, source_tx_id,
            shares_opened, shares_remaining,
            cost_price, buy_fee_total, buy_fee_remaining
        )
        VALUES (
            gen_random_uuid(), p_portfolio_id, p_company_id,
            NOW(), v_tx_id,
            p_shares, p_shares,
            p_price, p_fee, p_fee
        );

        INSERT INTO portfolio.holdings (portfolio_id, company_id, shares, avg_cost)
        VALUES (p_portfolio_id, p_company_id, p_shares, NULL)
        ON CONFLICT (portfolio_id, company_id)
        DO UPDATE SET shares = portfolio.holdings.shares + EXCLUDED.shares;

        UPDATE game.characters SET cash = cash - v_total_cost WHERE character_id = p_character_id;
        RETURN v_tx_id;
    END IF;

    -- SELL
    SELECT shares INTO v_hold_shares FROM portfolio.holdings
    WHERE portfolio_id = p_portfolio_id AND company_id = p_company_id FOR UPDATE;
    IF v_hold_shares IS NULL OR v_hold_shares < p_shares THEN
        RAISE EXCEPTION 'insufficient shares (need %, have %)', p_shares, COALESCE(v_hold_shares,0);
    END IF;

    INSERT INTO portfolio.transactions (tx_id, portfolio_id, company_id, side, shares, price, fee)
    VALUES (v_tx_id, p_portfolio_id, p_company_id, 'SELL', p_shares, p_price, p_fee);

    v_remaining := p_shares;
    v_sell_fee_per_sh := CASE WHEN p_fee = 0 THEN 0 ELSE p_fee / p_shares END;

    FOR rec IN
        SELECT * FROM portfolio.holding_lots
        WHERE portfolio_id = p_portfolio_id AND company_id = p_company_id AND shares_remaining > 0
        ORDER BY CASE WHEN v_lot_method = 'FIFO' THEN opened_at END ASC NULLS LAST,
                 CASE WHEN v_lot_method = 'LIFO' THEN opened_at END DESC NULLS LAST,
                 lot_id
        FOR UPDATE
    LOOP
        EXIT WHEN v_remaining <= 0;
        v_take := LEAST(v_remaining, rec.shares_remaining);
        v_lot_sh_before := rec.shares_remaining;
        v_buy_fee_per_sh := CASE WHEN rec.buy_fee_remaining > 0 AND v_lot_sh_before > 0
                                 THEN rec.buy_fee_remaining / v_lot_sh_before ELSE 0 END;
        v_buy_fee_take  := v_buy_fee_per_sh * v_take;
        v_sell_fee_take := v_sell_fee_per_sh * v_take;
        v_realized := ((p_price - rec.cost_price) * v_take) - v_buy_fee_take - v_sell_fee_take;

        INSERT INTO portfolio.lot_consumptions (
            consumption_id, sell_tx_id, lot_id,
            shares, buy_price, sell_price,
            buy_fee_alloc, sell_fee_alloc, realized_pnl
        )
        VALUES (gen_random_uuid(), v_tx_id, rec.lot_id,
                v_take, rec.cost_price, p_price,
                v_buy_fee_take, v_sell_fee_take, v_realized);

        UPDATE portfolio.holding_lots
        SET shares_remaining  = shares_remaining  - v_take,
            buy_fee_remaining = buy_fee_remaining - v_buy_fee_take
        WHERE lot_id = rec.lot_id;

        v_remaining := v_remaining - v_take;
    END LOOP;

    IF v_remaining > 0 THEN
        RAISE EXCEPTION 'internal: not enough lot shares to satisfy sell (remaining=%)', v_remaining;
    END IF;

    UPDATE portfolio.holdings SET shares = shares - p_shares, avg_cost = NULL
    WHERE portfolio_id = p_portfolio_id AND company_id = p_company_id;
    DELETE FROM portfolio.holdings WHERE portfolio_id = p_portfolio_id AND company_id = p_company_id AND shares <= 0;

    v_total_proceeds := (p_shares * p_price) - p_fee;
    UPDATE game.characters SET cash = cash + v_total_proceeds WHERE character_id = p_character_id;
    RETURN v_tx_id;
END;
$$ LANGUAGE plpgsql;

-- ==============================
-- execute_trade_internal (skips session token)
-- ==============================
CREATE OR REPLACE FUNCTION portfolio.execute_trade_internal(
    p_account_id    UUID,
    p_character_id  UUID,
    p_portfolio_id  UUID,
    p_company_id    BIGINT,
    p_side          TEXT,
    p_shares        NUMERIC(18,6),
    p_price         NUMERIC(18,6),
    p_fee           NUMERIC(18,6) DEFAULT 0
)
RETURNS UUID AS $$
DECLARE
    v_tx_id UUID;
    v_token TEXT := gen_random_uuid()::TEXT;
BEGIN
    IF p_account_id IS NULL THEN RAISE EXCEPTION 'unauthorized'; END IF;
    IF NOT EXISTS (SELECT 1 FROM game.characters c WHERE c.character_id = p_character_id AND c.account_id = p_account_id) THEN
        RAISE EXCEPTION 'forbidden: character not owned by account';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM portfolio.portfolios p WHERE p.portfolio_id = p_portfolio_id AND p.character_id = p_character_id) THEN
        RAISE EXCEPTION 'forbidden: portfolio not owned by character';
    END IF;

    PERFORM auth.new_session(p_account_id, v_token, 30, 'internal', NULL);
    BEGIN
        SELECT portfolio.execute_trade(v_token, p_character_id, p_portfolio_id, p_company_id, p_side, p_shares, p_price, p_fee) INTO v_tx_id;
        PERFORM auth.revoke_session(v_token);
        RETURN v_tx_id;
    EXCEPTION WHEN OTHERS THEN
        BEGIN PERFORM auth.revoke_session(v_token); EXCEPTION WHEN OTHERS THEN NULL; END;
        RAISE;
    END;
END;
$$ LANGUAGE plpgsql;

-- ==============================
-- cancel_order
-- ==============================
CREATE OR REPLACE FUNCTION portfolio.cancel_order(p_session_token TEXT, p_order_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    v_account_id UUID;
    v_ok BOOLEAN := FALSE;
BEGIN
    SELECT auth.verify_session(p_session_token) INTO v_account_id;
    IF v_account_id IS NULL THEN RAISE EXCEPTION 'unauthorized'; END IF;

    UPDATE portfolio.orders o
    SET status = 'CANCELLED', cancelled_at = NOW()
    WHERE o.order_id = p_order_id AND o.account_id = v_account_id AND o.status = 'OPEN'
    RETURNING TRUE INTO v_ok;

    RETURN COALESCE(v_ok, FALSE);
END;
$$ LANGUAGE plpgsql;

-- ==============================
-- place_order
-- ==============================
CREATE OR REPLACE FUNCTION portfolio.place_order(
    p_session_token TEXT,
    p_character_id  UUID,
    p_portfolio_id  UUID,
    p_company_id    BIGINT,
    p_side          TEXT,
    p_shares        NUMERIC(18,6),
    p_fee           NUMERIC(18,6) DEFAULT 0
)
RETURNS UUID AS $$
DECLARE
    v_account_id UUID;
    v_order_id   UUID := gen_random_uuid();
    v_sim_day    INTEGER;
    v_minute     INTEGER;
    v_is_open    BOOLEAN;
BEGIN
    IF p_shares IS NULL OR p_shares <= 0 THEN RAISE EXCEPTION 'shares must be > 0'; END IF;
    IF p_fee    IS NULL OR p_fee    <  0 THEN RAISE EXCEPTION 'fee must be >= 0';   END IF;
    IF p_side NOT IN ('BUY','SELL')      THEN RAISE EXCEPTION 'side must be BUY or SELL'; END IF;

    SELECT auth.verify_session(p_session_token) INTO v_account_id;
    IF v_account_id IS NULL THEN RAISE EXCEPTION 'unauthorized'; END IF;

    IF NOT EXISTS (SELECT 1 FROM game.characters c WHERE c.character_id = p_character_id AND c.account_id = v_account_id) THEN
        RAISE EXCEPTION 'forbidden: character not owned by account';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM portfolio.portfolios p WHERE p.portfolio_id = p_portfolio_id AND p.character_id = p_character_id) THEN
        RAISE EXCEPTION 'forbidden: portfolio not owned by character';
    END IF;

    SELECT sim_day, minute_of_day, is_open INTO v_sim_day, v_minute, v_is_open FROM market.get_clock();

    INSERT INTO portfolio.orders (
        order_id, account_id, character_id, portfolio_id,
        company_id, side, shares, fee,
        status, placed_sim_day, placed_minute
    )
    VALUES (
        v_order_id, v_account_id, p_character_id, p_portfolio_id,
        p_company_id, p_side, p_shares, p_fee,
        'OPEN', v_sim_day, v_minute
    );

    RETURN v_order_id;
END;
$$ LANGUAGE plpgsql;

-- ==============================
-- process_open_orders_for_tick
-- ==============================
CREATE OR REPLACE FUNCTION portfolio.process_open_orders_for_tick(p_sim_day INTEGER, p_minute INTEGER)
RETURNS INTEGER AS $$
DECLARE
    rec             RECORD;
    v_price         NUMERIC;
    v_tx_id         UUID;
    v_filled_count  INTEGER := 0;
BEGIN
    FOR rec IN
        SELECT * FROM portfolio.orders WHERE status = 'OPEN'
        ORDER BY placed_at ASC, order_id ASC
        FOR UPDATE SKIP LOCKED
    LOOP
        SELECT pt.price INTO v_price
        FROM market.prices_ticks pt
        WHERE pt.company_id = rec.company_id AND pt.sim_day = p_sim_day
        ORDER BY pt.ts DESC LIMIT 1;

        IF v_price IS NULL THEN
            SELECT pt.price INTO v_price
            FROM market.prices_ticks pt
            WHERE pt.company_id = rec.company_id
            ORDER BY pt.ts DESC LIMIT 1;
        END IF;

        IF v_price IS NULL OR v_price <= 0 THEN CONTINUE; END IF;

        BEGIN
            v_tx_id := portfolio.execute_trade_internal(
                rec.account_id, rec.character_id, rec.portfolio_id,
                rec.company_id, rec.side, rec.shares, v_price, rec.fee
            );

            UPDATE portfolio.orders
            SET status = 'FILLED', filled_at = NOW(),
                filled_sim_day = p_sim_day, filled_minute = p_minute,
                filled_price = v_price, tx_id = v_tx_id
            WHERE order_id = rec.order_id;

            v_filled_count := v_filled_count + 1;
        EXCEPTION WHEN OTHERS THEN
            UPDATE portfolio.orders
            SET status = 'REJECTED', filled_at = NOW(),
                filled_sim_day = p_sim_day, filled_minute = p_minute,
                filled_price = v_price, reject_reason = SQLERRM
            WHERE order_id = rec.order_id;
        END;
    END LOOP;

    RETURN v_filled_count;
END;
$$ LANGUAGE plpgsql;
