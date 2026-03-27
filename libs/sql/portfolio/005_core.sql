CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Convenience: default portfolio creation
CREATE OR REPLACE FUNCTION portfolio.create_default_portfolio(p_character_id UUID, p_name TEXT DEFAULT 'Main')
RETURNS UUID AS $$
DECLARE
    v_portfolio_id UUID;
BEGIN
    INSERT INTO portfolio.portfolios (character_id, name)
    VALUES (p_character_id, p_name)
    RETURNING portfolio_id INTO v_portfolio_id;

    RETURN v_portfolio_id;
END;
$$ LANGUAGE plpgsql;

-- ==========================================================
-- Server-authoritative trade execution (atomic)
-- ==========================================================
-- Inputs:
--  - session_token: bearer token from client (opaque)
--  - character_id: which character is trading
--  - portfolio_id: portfolio for that character
--  - company_id: what they trade
--  - side: BUY or SELL
--  - shares: quantity
--  - price: authoritative execution price
--  - fee: optional fee
--
-- Output:
--  - tx_id (uuid) of inserted transaction
--
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
    v_tx_id           UUID;

    v_cash            NUMERIC(18,6);
    v_total_cost      NUMERIC(18,6);

    v_cur_shares      NUMERIC(18,6);
    v_cur_avg_cost    NUMERIC(18,6);

    v_new_shares      NUMERIC(18,6);
    v_new_avg_cost    NUMERIC(18,6);

    v_portfolio_char  UUID;
BEGIN
    -- Basic input validation
    IF p_side NOT IN ('BUY', 'SELL') THEN
        RAISE EXCEPTION 'Invalid side: %', p_side;
    END IF;

    IF p_shares IS NULL OR p_shares <= 0 THEN
        RAISE EXCEPTION 'Shares must be > 0';
    END IF;

    IF p_price IS NULL OR p_price <= 0 THEN
        RAISE EXCEPTION 'Price must be > 0';
    END IF;

    IF p_fee IS NULL OR p_fee < 0 THEN
        RAISE EXCEPTION 'Fee must be >= 0';
    END IF;

    -- 1) Verify session -> account_id
    v_account_id := auth.verify_session(p_session_token);
    IF v_account_id IS NULL THEN
        RAISE EXCEPTION 'Unauthorized';
    END IF;

    -- 2) Verify account owns character
    IF NOT game.account_owns_character(v_account_id, p_character_id) THEN
        RAISE EXCEPTION 'Forbidden (character not owned)';
    END IF;

    -- 3) Verify portfolio belongs to character
    SELECT character_id
      INTO v_portfolio_char
    FROM portfolio.portfolios
    WHERE portfolio_id = p_portfolio_id;

    IF v_portfolio_char IS NULL THEN
        RAISE EXCEPTION 'Portfolio not found';
    END IF;

    IF v_portfolio_char <> p_character_id THEN
        RAISE EXCEPTION 'Forbidden (portfolio not owned by character)';
    END IF;

    -- 4) Lock character row (cash) for update
    SELECT cash
      INTO v_cash
    FROM game.characters
    WHERE character_id = p_character_id
    FOR UPDATE;

    IF v_cash IS NULL THEN
        RAISE EXCEPTION 'Character not found';
    END IF;

    v_total_cost := (p_shares * p_price) + p_fee;

    -- 5) Lock holdings row (if exists) for update
    SELECT shares, avg_cost
      INTO v_cur_shares, v_cur_avg_cost
    FROM portfolio.holdings
    WHERE portfolio_id = p_portfolio_id
      AND company_id = p_company_id
    FOR UPDATE;

    IF NOT FOUND THEN
        v_cur_shares := 0;
        v_cur_avg_cost := NULL;
    END IF;

    -- 6) Apply trade rules
    IF p_side = 'BUY' THEN
        IF v_cash < v_total_cost THEN
            RAISE EXCEPTION 'Insufficient cash (have %, need %)', v_cash, v_total_cost;
        END IF;

        v_new_shares := v_cur_shares + p_shares;

        -- weighted avg cost:
        -- new_avg = (old_shares*old_avg + buy_shares*price) / new_shares
        IF v_cur_shares = 0 OR v_cur_avg_cost IS NULL THEN
            v_new_avg_cost := p_price;
        ELSE
            v_new_avg_cost := ((v_cur_shares * v_cur_avg_cost) + (p_shares * p_price)) / v_new_shares;
        END IF;

        -- debit cash
        UPDATE game.characters
        SET cash = cash - v_total_cost
        WHERE character_id = p_character_id;

    ELSE
        -- SELL
        IF v_cur_shares < p_shares THEN
            RAISE EXCEPTION 'Insufficient shares (have %, trying to sell %)', v_cur_shares, p_shares;
        END IF;

        v_new_shares := v_cur_shares - p_shares;

        -- avg_cost typically unchanged on sell; if position closed, null it out
        IF v_new_shares = 0 THEN
            v_new_avg_cost := NULL;
        ELSE
            v_new_avg_cost := v_cur_avg_cost;
        END IF;

        -- credit cash: proceeds - fee
        UPDATE game.characters
        SET cash = cash + (p_shares * p_price) - p_fee
        WHERE character_id = p_character_id;
    END IF;

    -- 7) Insert transaction
    INSERT INTO portfolio.transactions (portfolio_id, company_id, side, shares, price, fee)
    VALUES (p_portfolio_id, p_company_id, p_side, p_shares, p_price, p_fee)
    RETURNING tx_id INTO v_tx_id;

    -- 8) Upsert holdings (or delete if closed)
    IF v_new_shares = 0 THEN
        DELETE FROM portfolio.holdings
        WHERE portfolio_id = p_portfolio_id
          AND company_id = p_company_id;
    ELSE
        INSERT INTO portfolio.holdings (portfolio_id, company_id, shares, avg_cost)
        VALUES (p_portfolio_id, p_company_id, v_new_shares, v_new_avg_cost)
        ON CONFLICT (portfolio_id, company_id)
        DO UPDATE SET
            shares = EXCLUDED.shares,
            avg_cost = EXCLUDED.avg_cost;
    END IF;

    RETURN v_tx_id;
END;
$$ LANGUAGE plpgsql;