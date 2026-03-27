--
-- PostgreSQL database dump
--

\restrict DjzfVWS1trAzQvhywtnDDEMhRb80DgK7MIt43EisIIAIpQb6cUcfpag6865tDwq

-- Dumped from database version 17.6
-- Dumped by pg_dump version 17.6

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: portfolio; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA portfolio;


--
-- Name: lot_method; Type: TYPE; Schema: portfolio; Owner: -
--

CREATE TYPE portfolio.lot_method AS ENUM (
    'FIFO',
    'LIFO'
);


--
-- Name: order_status; Type: TYPE; Schema: portfolio; Owner: -
--

CREATE TYPE portfolio.order_status AS ENUM (
    'OPEN',
    'FILLED',
    'CANCELLED',
    'REJECTED'
);


--
-- Name: cancel_order(text, uuid); Type: FUNCTION; Schema: portfolio; Owner: -
--

CREATE FUNCTION portfolio.cancel_order(p_session_token text, p_order_id uuid) RETURNS boolean
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_account_id UUID;
    v_ok BOOLEAN := FALSE;
BEGIN
    SELECT auth.verify_session(p_session_token) INTO v_account_id;
    IF v_account_id IS NULL THEN
        RAISE EXCEPTION 'unauthorized';
    END IF;

    UPDATE portfolio.orders o
    SET status = 'CANCELLED',
        cancelled_at = NOW()
    WHERE o.order_id = p_order_id
      AND o.account_id = v_account_id
      AND o.status = 'OPEN'
    RETURNING TRUE INTO v_ok;

    RETURN COALESCE(v_ok, FALSE);
END;
$$;


--
-- Name: create_default_portfolio(uuid, text); Type: FUNCTION; Schema: portfolio; Owner: -
--

CREATE FUNCTION portfolio.create_default_portfolio(p_character_id uuid, p_name text DEFAULT 'Main'::text) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_portfolio_id UUID;
BEGIN
    INSERT INTO portfolio.portfolios (character_id, name)
    VALUES (p_character_id, p_name)
    RETURNING portfolio_id INTO v_portfolio_id;

    RETURN v_portfolio_id;
END;
$$;


--
-- Name: execute_trade(text, uuid, uuid, bigint, text, numeric, numeric, numeric); Type: FUNCTION; Schema: portfolio; Owner: -
--

CREATE FUNCTION portfolio.execute_trade(p_session_token text, p_character_id uuid, p_portfolio_id uuid, p_company_id bigint, p_side text, p_shares numeric, p_price numeric, p_fee numeric DEFAULT 0) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
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
    IF p_shares IS NULL OR p_shares <= 0 THEN
        RAISE EXCEPTION 'shares must be > 0';
    END IF;

    IF p_price IS NULL OR p_price <= 0 THEN
        RAISE EXCEPTION 'price must be > 0';
    END IF;

    IF p_fee IS NULL OR p_fee < 0 THEN
        RAISE EXCEPTION 'fee must be >= 0';
    END IF;

    IF p_side NOT IN ('BUY','SELL') THEN
        RAISE EXCEPTION 'side must be BUY or SELL';
    END IF;

    -- verify session
    SELECT auth.verify_session(p_session_token) INTO v_account_id;
    IF v_account_id IS NULL THEN
        RAISE EXCEPTION 'unauthorized';
    END IF;

    -- ownership checks
    IF NOT EXISTS (
        SELECT 1
        FROM game.characters c
        WHERE c.character_id = p_character_id
          AND c.account_id = v_account_id
    ) THEN
        RAISE EXCEPTION 'forbidden: character not owned by account';
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM portfolio.portfolios p
        WHERE p.portfolio_id = p_portfolio_id
          AND p.character_id = p_character_id
    ) THEN
        RAISE EXCEPTION 'forbidden: portfolio not owned by character';
    END IF;

    SELECT p.lot_method
    INTO v_lot_method
    FROM portfolio.portfolios p
    WHERE p.portfolio_id = p_portfolio_id;

    -- lock character row for cash updates
    SELECT cash
      INTO v_cash
    FROM game.characters
    WHERE character_id = p_character_id
    FOR UPDATE;

    IF p_side = 'BUY' THEN
        v_total_cost := (p_shares * p_price) + p_fee;

        IF v_cash < v_total_cost THEN
            RAISE EXCEPTION 'insufficient cash (need %, have %)', v_total_cost, v_cash;
        END IF;

        INSERT INTO portfolio.transactions (tx_id, portfolio_id, company_id, side, shares, price, fee)
        VALUES (v_tx_id, p_portfolio_id, p_company_id, 'BUY', p_shares, p_price, p_fee);

        INSERT INTO portfolio.holding_lots (
            lot_id, portfolio_id, company_id,
            opened_at, source_buy_tx_id,
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

        UPDATE game.characters
        SET cash = cash - v_total_cost
        WHERE character_id = p_character_id;

        RETURN v_tx_id;
    END IF;

    -- SELL
    SELECT shares
      INTO v_hold_shares
    FROM portfolio.holdings
    WHERE portfolio_id = p_portfolio_id
      AND company_id = p_company_id
    FOR UPDATE;

    IF v_hold_shares IS NULL OR v_hold_shares < p_shares THEN
        RAISE EXCEPTION 'insufficient shares (need %, have %)', p_shares, COALESCE(v_hold_shares, 0);
    END IF;

    INSERT INTO portfolio.transactions (tx_id, portfolio_id, company_id, side, shares, price, fee)
    VALUES (v_tx_id, p_portfolio_id, p_company_id, 'SELL', p_shares, p_price, p_fee);

    v_remaining := p_shares;

    IF p_fee = 0 THEN
        v_sell_fee_per_sh := 0;
    ELSE
        v_sell_fee_per_sh := p_fee / p_shares;
    END IF;

    IF v_lot_method = 'FIFO' THEN
        FOR rec IN
            SELECT *
            FROM portfolio.holding_lots
            WHERE portfolio_id = p_portfolio_id
              AND company_id = p_company_id
              AND shares_remaining > 0
            ORDER BY opened_at ASC, lot_id ASC
            FOR UPDATE
        LOOP
            EXIT WHEN v_remaining <= 0;

            v_take := LEAST(v_remaining, rec.shares_remaining);
            v_lot_sh_before := rec.shares_remaining;

            IF rec.buy_fee_remaining > 0 AND v_lot_sh_before > 0 THEN
                v_buy_fee_per_sh := rec.buy_fee_remaining / v_lot_sh_before;
                v_buy_fee_take := v_buy_fee_per_sh * v_take;
            ELSE
                v_buy_fee_take := 0;
            END IF;

            v_sell_fee_take := v_sell_fee_per_sh * v_take;
            v_realized := ((p_price - rec.cost_price) * v_take) - v_buy_fee_take - v_sell_fee_take;

            INSERT INTO portfolio.lot_consumptions (
                consumption_id, sell_tx_id, lot_id,
                shares, buy_price, sell_price,
                buy_fee_alloc, sell_fee_alloc,
                realized_pnl
            )
            VALUES (
                gen_random_uuid(), v_tx_id, rec.lot_id,
                v_take, rec.cost_price, p_price,
                v_buy_fee_take, v_sell_fee_take,
                v_realized
            );

            UPDATE portfolio.holding_lots
            SET shares_remaining  = shares_remaining - v_take,
                buy_fee_remaining = buy_fee_remaining - v_buy_fee_take
            WHERE lot_id = rec.lot_id;

            v_remaining := v_remaining - v_take;
        END LOOP;
    ELSE
        FOR rec IN
            SELECT *
            FROM portfolio.holding_lots
            WHERE portfolio_id = p_portfolio_id
              AND company_id = p_company_id
              AND shares_remaining > 0
            ORDER BY opened_at DESC, lot_id DESC
            FOR UPDATE
        LOOP
            EXIT WHEN v_remaining <= 0;

            v_take := LEAST(v_remaining, rec.shares_remaining);
            v_lot_sh_before := rec.shares_remaining;

            IF rec.buy_fee_remaining > 0 AND v_lot_sh_before > 0 THEN
                v_buy_fee_per_sh := rec.buy_fee_remaining / v_lot_sh_before;
                v_buy_fee_take := v_buy_fee_per_sh * v_take;
            ELSE
                v_buy_fee_take := 0;
            END IF;

            v_sell_fee_take := v_sell_fee_per_sh * v_take;
            v_realized := ((p_price - rec.cost_price) * v_take) - v_buy_fee_take - v_sell_fee_take;

            INSERT INTO portfolio.lot_consumptions (
                consumption_id, sell_tx_id, lot_id,
                shares, buy_price, sell_price,
                buy_fee_alloc, sell_fee_alloc,
                realized_pnl
            )
            VALUES (
                gen_random_uuid(), v_tx_id, rec.lot_id,
                v_take, rec.cost_price, p_price,
                v_buy_fee_take, v_sell_fee_take,
                v_realized
            );

            UPDATE portfolio.holding_lots
            SET shares_remaining  = shares_remaining - v_take,
                buy_fee_remaining = buy_fee_remaining - v_buy_fee_take
            WHERE lot_id = rec.lot_id;

            v_remaining := v_remaining - v_take;
        END LOOP;
    END IF;

    IF v_remaining > 0 THEN
        RAISE EXCEPTION 'internal: not enough lot shares to satisfy sell (remaining=%)', v_remaining;
    END IF;

    UPDATE portfolio.holdings
    SET shares = shares - p_shares,
        avg_cost = NULL
    WHERE portfolio_id = p_portfolio_id
      AND company_id = p_company_id;

    DELETE FROM portfolio.holdings
    WHERE portfolio_id = p_portfolio_id
      AND company_id = p_company_id
      AND shares <= 0;

    v_total_proceeds := (p_shares * p_price) - p_fee;

    UPDATE game.characters
    SET cash = cash + v_total_proceeds
    WHERE character_id = p_character_id;

    RETURN v_tx_id;
END;
$$;


--
-- Name: execute_trade_internal(uuid, uuid, uuid, bigint, text, numeric, numeric, numeric); Type: FUNCTION; Schema: portfolio; Owner: -
--

CREATE FUNCTION portfolio.execute_trade_internal(p_account_id uuid, p_character_id uuid, p_portfolio_id uuid, p_company_id bigint, p_side text, p_shares numeric, p_price numeric, p_fee numeric DEFAULT 0) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_tx_id UUID;
BEGIN
    IF p_account_id IS NULL THEN
        RAISE EXCEPTION 'unauthorized';
    END IF;

    -- ownership checks
    IF NOT EXISTS (
        SELECT 1
        FROM game.characters c
        WHERE c.character_id = p_character_id
          AND c.account_id = p_account_id
    ) THEN
        RAISE EXCEPTION 'forbidden: character not owned by account';
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM portfolio.portfolios p
        WHERE p.portfolio_id = p_portfolio_id
          AND p.character_id = p_character_id
    ) THEN
        RAISE EXCEPTION 'forbidden: portfolio not owned by character';
    END IF;

    -- Call your main execution function by using a special internal session token?
    -- NO: we avoid sessions entirely here by routing directly into the logic.
    --
    -- Easiest/cleanest: re-use your execute_trade() body in this function.
    -- But you already have it implemented, so instead we do:
    --  - Create a temporary session row? (messy)
    --  - Or: duplicate logic again (also messy)
    --
    -- ✅ Practical compromise:
    -- We assume your execute_trade() function is the canonical implementation and
    -- we "bridge" by creating a short-lived session token row.
    --
    -- If you do not want this, tell me and I’ll give you the pure duplicated-body version.

    -- ---- bridge session (short-lived) ----
    -- token is random uuid text, stored hashed in auth.sessions
    PERFORM 1;

    -- create an ephemeral session token
    -- (expires in 30 seconds; immediately revoked after use)
    -- NOTE: requires auth.new_session(...) and auth.revoke_session(...) exist
    -- from your auth migration.
    DECLARE
        v_token TEXT := gen_random_uuid()::text;
        v_session_id UUID;
    BEGIN
        SELECT auth.new_session(p_account_id, v_token, 30, 'internal', NULL) INTO v_session_id;
        SELECT portfolio.execute_trade(
            v_token,
            p_character_id,
            p_portfolio_id,
            p_company_id,
            p_side,
            p_shares,
            p_price,
            p_fee
        ) INTO v_tx_id;

        PERFORM auth.revoke_session(v_token);
        RETURN v_tx_id;
    EXCEPTION WHEN OTHERS THEN
        -- best-effort revoke if created
        BEGIN
          PERFORM auth.revoke_session(v_token);
        EXCEPTION WHEN OTHERS THEN
          NULL;
        END;
        RAISE;
    END;

END;
$$;


--
-- Name: place_order(text, uuid, uuid, bigint, text, numeric, numeric); Type: FUNCTION; Schema: portfolio; Owner: -
--

CREATE FUNCTION portfolio.place_order(p_session_token text, p_character_id uuid, p_portfolio_id uuid, p_company_id bigint, p_side text, p_shares numeric, p_fee numeric DEFAULT 0) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_account_id UUID;
    v_order_id UUID := gen_random_uuid();
    v_sim_day INTEGER;
    v_minute INTEGER;
    v_is_open BOOLEAN;
BEGIN
    IF p_shares IS NULL OR p_shares <= 0 THEN
        RAISE EXCEPTION 'shares must be > 0';
    END IF;

    IF p_fee IS NULL OR p_fee < 0 THEN
        RAISE EXCEPTION 'fee must be >= 0';
    END IF;

    IF p_side NOT IN ('BUY','SELL') THEN
        RAISE EXCEPTION 'side must be BUY or SELL';
    END IF;

    SELECT auth.verify_session(p_session_token) INTO v_account_id;
    IF v_account_id IS NULL THEN
        RAISE EXCEPTION 'unauthorized';
    END IF;

    -- ownership checks
    IF NOT EXISTS (
        SELECT 1
        FROM game.characters c
        WHERE c.character_id = p_character_id
          AND c.account_id = v_account_id
    ) THEN
        RAISE EXCEPTION 'forbidden: character not owned by account';
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM portfolio.portfolios p
        WHERE p.portfolio_id = p_portfolio_id
          AND p.character_id = p_character_id
    ) THEN
        RAISE EXCEPTION 'forbidden: portfolio not owned by character';
    END IF;

    SELECT sim_day, minute_of_day, is_open
    INTO v_sim_day, v_minute, v_is_open
    FROM market.get_clock();

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
$$;


--
-- Name: process_open_orders_for_tick(integer, integer); Type: FUNCTION; Schema: portfolio; Owner: -
--

CREATE FUNCTION portfolio.process_open_orders_for_tick(p_sim_day integer, p_minute integer) RETURNS integer
    LANGUAGE plpgsql
    AS $$
DECLARE
    rec RECORD;
    v_price NUMERIC;
    v_tx_id UUID;
    v_filled_count INTEGER := 0;
BEGIN
    FOR rec IN
        SELECT *
        FROM portfolio.orders
        WHERE status = 'OPEN'
        ORDER BY placed_at ASC, order_id ASC
        FOR UPDATE SKIP LOCKED
    LOOP
        -- price lookup: ticks for current sim_day first
        SELECT pt.price
        INTO v_price
        FROM market.prices_ticks pt
        WHERE pt.company_id = rec.company_id
          AND pt.sim_day = p_sim_day
        ORDER BY pt.ts DESC
        LIMIT 1;

        IF v_price IS NULL THEN
            -- fallback: latest tick overall
            SELECT pt.price
            INTO v_price
            FROM market.prices_ticks pt
            WHERE pt.company_id = rec.company_id
            ORDER BY pt.ts DESC
            LIMIT 1;
        END IF;

        IF v_price IS NULL OR v_price <= 0 THEN
            -- no price available yet -> leave OPEN (try again next tick)
            CONTINUE;
        END IF;

        BEGIN
            v_tx_id := portfolio.execute_trade_internal(
                rec.account_id,
                rec.character_id,
                rec.portfolio_id,
                rec.company_id,
                rec.side,
                rec.shares,
                v_price,
                rec.fee
            );

            UPDATE portfolio.orders
            SET status = 'FILLED',
                filled_at = NOW(),
                filled_sim_day = p_sim_day,
                filled_minute = p_minute,
                filled_price = v_price,
                tx_id = v_tx_id
            WHERE order_id = rec.order_id;

            v_filled_count := v_filled_count + 1;

        EXCEPTION WHEN OTHERS THEN
            UPDATE portfolio.orders
            SET status = 'REJECTED',
                filled_at = NOW(),
                filled_sim_day = p_sim_day,
                filled_minute = p_minute,
                filled_price = v_price,
                reject_reason = SQLERRM
            WHERE order_id = rec.order_id;
        END;
    END LOOP;

    RETURN v_filled_count;
END;
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: holding_lots; Type: TABLE; Schema: portfolio; Owner: -
--

CREATE TABLE portfolio.holding_lots (
    lot_id uuid NOT NULL,
    portfolio_id uuid NOT NULL,
    company_id bigint NOT NULL,
    opened_at timestamp without time zone DEFAULT now() NOT NULL,
    source_tx_id uuid NOT NULL,
    shares_opened numeric(18,6) NOT NULL,
    shares_remaining numeric(18,6) NOT NULL,
    cost_price numeric(18,6) NOT NULL,
    fee_allocated numeric(18,6) DEFAULT 0 NOT NULL,
    buy_fee_total numeric(18,6) DEFAULT 0 NOT NULL,
    buy_fee_remaining numeric(18,6) DEFAULT 0 NOT NULL
);


--
-- Name: holdings; Type: TABLE; Schema: portfolio; Owner: -
--

CREATE TABLE portfolio.holdings (
    portfolio_id uuid NOT NULL,
    company_id bigint NOT NULL,
    shares numeric(18,6) DEFAULT 0 NOT NULL,
    avg_cost numeric(18,6)
);


--
-- Name: lot_consumptions; Type: TABLE; Schema: portfolio; Owner: -
--

CREATE TABLE portfolio.lot_consumptions (
    consumption_id uuid NOT NULL,
    sell_tx_id uuid NOT NULL,
    lot_id uuid NOT NULL,
    shares numeric(18,6) NOT NULL,
    buy_price numeric(18,6) NOT NULL,
    sell_price numeric(18,6) NOT NULL,
    realized_pnl numeric(18,6) NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    buy_fee_alloc numeric(18,6) DEFAULT 0 NOT NULL,
    sell_fee_alloc numeric(18,6) DEFAULT 0 NOT NULL
);


--
-- Name: orders; Type: TABLE; Schema: portfolio; Owner: -
--

CREATE TABLE portfolio.orders (
    order_id uuid DEFAULT gen_random_uuid() NOT NULL,
    account_id uuid NOT NULL,
    character_id uuid NOT NULL,
    portfolio_id uuid NOT NULL,
    company_id bigint NOT NULL,
    side text NOT NULL,
    shares numeric(18,6) NOT NULL,
    fee numeric(18,6) DEFAULT 0 NOT NULL,
    status portfolio.order_status DEFAULT 'OPEN'::portfolio.order_status NOT NULL,
    reject_reason text,
    placed_at timestamp without time zone DEFAULT now() NOT NULL,
    placed_sim_day integer,
    placed_minute integer,
    filled_at timestamp without time zone,
    filled_sim_day integer,
    filled_minute integer,
    filled_price numeric(18,6),
    tx_id uuid,
    cancelled_at timestamp without time zone
);


--
-- Name: portfolios; Type: TABLE; Schema: portfolio; Owner: -
--

CREATE TABLE portfolio.portfolios (
    portfolio_id uuid DEFAULT gen_random_uuid() NOT NULL,
    character_id uuid NOT NULL,
    name text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    lot_method portfolio.lot_method DEFAULT 'FIFO'::portfolio.lot_method NOT NULL
);


--
-- Name: transactions; Type: TABLE; Schema: portfolio; Owner: -
--

CREATE TABLE portfolio.transactions (
    tx_id uuid DEFAULT gen_random_uuid() NOT NULL,
    portfolio_id uuid NOT NULL,
    company_id bigint NOT NULL,
    ts timestamp without time zone DEFAULT now() NOT NULL,
    side text NOT NULL,
    shares numeric(18,6) NOT NULL,
    price numeric(18,6) NOT NULL,
    fee numeric(18,6) DEFAULT 0 NOT NULL,
    CONSTRAINT transactions_side_chk CHECK ((side = ANY (ARRAY['BUY'::text, 'SELL'::text])))
);


--
-- Name: v_positions; Type: VIEW; Schema: portfolio; Owner: -
--

CREATE VIEW portfolio.v_positions AS
 WITH lot_rollup AS (
         SELECT l.portfolio_id,
            l.company_id,
            sum(l.shares_remaining) AS shares,
            sum(((l.cost_price * l.shares_remaining) + l.buy_fee_remaining)) AS cost_basis_remaining
           FROM portfolio.holding_lots l
          WHERE (l.shares_remaining > (0)::numeric)
          GROUP BY l.portfolio_id, l.company_id
        ), latest_prices AS (
         SELECT DISTINCT ON (p.company_id) p.company_id,
            (p.price)::numeric(18,6) AS price,
            p.ts
           FROM market.prices p
          ORDER BY p.company_id, p.ts DESC
        )
 SELECT r.portfolio_id,
    r.company_id,
    r.shares,
        CASE
            WHEN (r.shares > (0)::numeric) THEN (r.cost_basis_remaining / r.shares)
            ELSE NULL::numeric
        END AS avg_cost_fee_aware,
    lp.price AS current_price,
        CASE
            WHEN (lp.price IS NOT NULL) THEN ((lp.price * r.shares) - r.cost_basis_remaining)
            ELSE NULL::numeric
        END AS unrealized_pnl
   FROM (lot_rollup r
     LEFT JOIN latest_prices lp ON ((lp.company_id = r.company_id)));


--
-- Name: holding_lots holding_lots_pkey; Type: CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.holding_lots
    ADD CONSTRAINT holding_lots_pkey PRIMARY KEY (lot_id);


--
-- Name: holdings holdings_pkey; Type: CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.holdings
    ADD CONSTRAINT holdings_pkey PRIMARY KEY (portfolio_id, company_id);


--
-- Name: lot_consumptions lot_consumptions_pkey; Type: CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.lot_consumptions
    ADD CONSTRAINT lot_consumptions_pkey PRIMARY KEY (consumption_id);


--
-- Name: orders orders_pkey; Type: CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.orders
    ADD CONSTRAINT orders_pkey PRIMARY KEY (order_id);


--
-- Name: portfolios portfolios_pkey; Type: CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.portfolios
    ADD CONSTRAINT portfolios_pkey PRIMARY KEY (portfolio_id);


--
-- Name: transactions transactions_pkey; Type: CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.transactions
    ADD CONSTRAINT transactions_pkey PRIMARY KEY (tx_id);


--
-- Name: idx_holdings_portfolio; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_holdings_portfolio ON portfolio.holdings USING btree (portfolio_id);


--
-- Name: idx_lot_consumptions_lot; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_lot_consumptions_lot ON portfolio.lot_consumptions USING btree (lot_id);


--
-- Name: idx_lot_consumptions_sell_tx; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_lot_consumptions_sell_tx ON portfolio.lot_consumptions USING btree (sell_tx_id);


--
-- Name: idx_lots_portfolio_company_opened; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_lots_portfolio_company_opened ON portfolio.holding_lots USING btree (portfolio_id, company_id, opened_at);


--
-- Name: idx_lots_portfolio_company_remaining; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_lots_portfolio_company_remaining ON portfolio.holding_lots USING btree (portfolio_id, company_id, shares_remaining);


--
-- Name: idx_lots_remaining; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_lots_remaining ON portfolio.holding_lots USING btree (portfolio_id, company_id, shares_remaining);


--
-- Name: idx_orders_account_status_time; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_orders_account_status_time ON portfolio.orders USING btree (account_id, status, placed_at DESC);


--
-- Name: idx_orders_company_status_time; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_orders_company_status_time ON portfolio.orders USING btree (company_id, status, placed_at DESC);


--
-- Name: idx_orders_portfolio_status_time; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_orders_portfolio_status_time ON portfolio.orders USING btree (portfolio_id, status, placed_at DESC);


--
-- Name: idx_portfolios_character; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_portfolios_character ON portfolio.portfolios USING btree (character_id);


--
-- Name: idx_transactions_company; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_transactions_company ON portfolio.transactions USING btree (company_id);


--
-- Name: idx_transactions_portfolio_ts; Type: INDEX; Schema: portfolio; Owner: -
--

CREATE INDEX idx_transactions_portfolio_ts ON portfolio.transactions USING btree (portfolio_id, ts DESC);


--
-- Name: holding_lots holding_lots_company_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.holding_lots
    ADD CONSTRAINT holding_lots_company_id_fkey FOREIGN KEY (company_id) REFERENCES market.companies(company_id);


--
-- Name: holding_lots holding_lots_portfolio_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.holding_lots
    ADD CONSTRAINT holding_lots_portfolio_id_fkey FOREIGN KEY (portfolio_id) REFERENCES portfolio.portfolios(portfolio_id);


--
-- Name: holding_lots holding_lots_source_tx_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.holding_lots
    ADD CONSTRAINT holding_lots_source_tx_id_fkey FOREIGN KEY (source_tx_id) REFERENCES portfolio.transactions(tx_id);


--
-- Name: holdings holdings_company_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.holdings
    ADD CONSTRAINT holdings_company_id_fkey FOREIGN KEY (company_id) REFERENCES market.companies(company_id);


--
-- Name: holdings holdings_portfolio_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.holdings
    ADD CONSTRAINT holdings_portfolio_id_fkey FOREIGN KEY (portfolio_id) REFERENCES portfolio.portfolios(portfolio_id) ON DELETE CASCADE;


--
-- Name: lot_consumptions lot_consumptions_lot_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.lot_consumptions
    ADD CONSTRAINT lot_consumptions_lot_id_fkey FOREIGN KEY (lot_id) REFERENCES portfolio.holding_lots(lot_id);


--
-- Name: lot_consumptions lot_consumptions_sell_tx_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.lot_consumptions
    ADD CONSTRAINT lot_consumptions_sell_tx_id_fkey FOREIGN KEY (sell_tx_id) REFERENCES portfolio.transactions(tx_id);


--
-- Name: orders orders_account_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.orders
    ADD CONSTRAINT orders_account_id_fkey FOREIGN KEY (account_id) REFERENCES auth.accounts(account_id);


--
-- Name: orders orders_character_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.orders
    ADD CONSTRAINT orders_character_id_fkey FOREIGN KEY (character_id) REFERENCES game.characters(character_id);


--
-- Name: orders orders_company_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.orders
    ADD CONSTRAINT orders_company_id_fkey FOREIGN KEY (company_id) REFERENCES market.companies(company_id);


--
-- Name: orders orders_portfolio_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.orders
    ADD CONSTRAINT orders_portfolio_id_fkey FOREIGN KEY (portfolio_id) REFERENCES portfolio.portfolios(portfolio_id);


--
-- Name: orders orders_tx_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.orders
    ADD CONSTRAINT orders_tx_id_fkey FOREIGN KEY (tx_id) REFERENCES portfolio.transactions(tx_id);


--
-- Name: portfolios portfolios_character_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.portfolios
    ADD CONSTRAINT portfolios_character_id_fkey FOREIGN KEY (character_id) REFERENCES game.characters(character_id) ON DELETE CASCADE;


--
-- Name: transactions transactions_company_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.transactions
    ADD CONSTRAINT transactions_company_id_fkey FOREIGN KEY (company_id) REFERENCES market.companies(company_id);


--
-- Name: transactions transactions_portfolio_id_fkey; Type: FK CONSTRAINT; Schema: portfolio; Owner: -
--

ALTER TABLE ONLY portfolio.transactions
    ADD CONSTRAINT transactions_portfolio_id_fkey FOREIGN KEY (portfolio_id) REFERENCES portfolio.portfolios(portfolio_id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

\unrestrict DjzfVWS1trAzQvhywtnDDEMhRb80DgK7MIt43EisIIAIpQb6cUcfpag6865tDwq

