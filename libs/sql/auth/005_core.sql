-- updated_at trigger
CREATE OR REPLACE FUNCTION auth.touch_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_auth_accounts_touch ON auth.accounts;
CREATE TRIGGER trg_auth_accounts_touch
BEFORE UPDATE ON auth.accounts
FOR EACH ROW
EXECUTE FUNCTION auth.touch_updated_at();

-- ------------------------------
-- Role seeds
-- ------------------------------
INSERT INTO auth.roles (role_name)
VALUES ('player'), ('admin')
ON CONFLICT (role_name) DO NOTHING;

-- ------------------------------
-- Password helpers (bcrypt)
-- ------------------------------
CREATE OR REPLACE FUNCTION auth.hash_password(p_password TEXT, p_cost INT DEFAULT 12)
RETURNS TEXT AS $$
BEGIN
    RETURN crypt(p_password, gen_salt('bf', p_cost));
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION auth.verify_password(p_password TEXT, p_hash TEXT)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN (p_hash = crypt(p_password, p_hash));
END;
$$ LANGUAGE plpgsql;

-- ------------------------------
-- Auth login: returns account_id if valid
-- ------------------------------
CREATE OR REPLACE FUNCTION auth.login(p_email TEXT, p_password TEXT)
RETURNS UUID AS $$
DECLARE
    v_account_id UUID;
    v_hash TEXT;
BEGIN
    SELECT account_id, password_hash
      INTO v_account_id, v_hash
    FROM auth.accounts
    WHERE email = p_email
      AND is_disabled = FALSE;

    IF v_account_id IS NULL THEN
        RETURN NULL;
    END IF;

    IF auth.verify_password(p_password, v_hash) THEN
        UPDATE auth.accounts SET last_login_at = NOW() WHERE account_id = v_account_id;
        RETURN v_account_id;
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- ------------------------------
-- Session issuance / verification
-- ------------------------------
CREATE OR REPLACE FUNCTION auth.new_session(
    p_account_id UUID,
    p_token TEXT,
    p_ttl_seconds INT,
    p_user_agent TEXT DEFAULT NULL,
    p_ip INET DEFAULT NULL
)
RETURNS UUID AS $$
DECLARE
    v_session_id UUID;
BEGIN
    INSERT INTO auth.sessions (account_id, token_hash, expires_at, user_agent, ip_addr)
    VALUES (
        p_account_id,
        digest(p_token, 'sha256'),
        NOW() + make_interval(secs => p_ttl_seconds),
        p_user_agent,
        p_ip
    )
    RETURNING session_id INTO v_session_id;

    RETURN v_session_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION auth.verify_session(p_token TEXT)
RETURNS UUID AS $$
DECLARE
    v_account_id UUID;
BEGIN
    SELECT s.account_id
      INTO v_account_id
    FROM auth.sessions s
    JOIN auth.accounts a ON a.account_id = s.account_id
    WHERE s.token_hash = digest(p_token, 'sha256')
      AND s.revoked_at IS NULL
      AND s.expires_at > NOW()
      AND a.is_disabled = FALSE;

    RETURN v_account_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION auth.revoke_session(p_token TEXT)
RETURNS VOID AS $$
BEGIN
    UPDATE auth.sessions
    SET revoked_at = NOW()
    WHERE token_hash = digest(p_token, 'sha256')
      AND revoked_at IS NULL;
END;
$$ LANGUAGE plpgsql;

-- ------------------------------
-- Refresh issuance / verification / rotation support
-- ------------------------------
CREATE OR REPLACE FUNCTION auth.new_refresh(
    p_account_id UUID,
    p_token TEXT,
    p_ttl_seconds INT,
    p_user_agent TEXT DEFAULT NULL,
    p_ip INET DEFAULT NULL
)
RETURNS UUID AS $$
DECLARE
    v_refresh_id UUID;
BEGIN
    INSERT INTO auth.refresh_tokens (account_id, token_hash, expires_at, user_agent, ip_addr)
    VALUES (
        p_account_id,
        digest(p_token, 'sha256'),
        NOW() + make_interval(secs => p_ttl_seconds),
        p_user_agent,
        p_ip
    )
    RETURNING refresh_id INTO v_refresh_id;

    RETURN v_refresh_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION auth.verify_refresh(p_token TEXT)
RETURNS UUID AS $$
DECLARE
    v_account_id UUID;
BEGIN
    SELECT r.account_id
      INTO v_account_id
    FROM auth.refresh_tokens r
    JOIN auth.accounts a ON a.account_id = r.account_id
    WHERE r.token_hash = digest(p_token, 'sha256')
      AND r.revoked_at IS NULL
      AND r.expires_at > NOW()
      AND a.is_disabled = FALSE;

    RETURN v_account_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION auth.rotate_refresh(p_old_token TEXT, p_new_refresh_id UUID)
RETURNS VOID AS $$
BEGIN
    UPDATE auth.refresh_tokens
    SET revoked_at = NOW(),
        replaced_by_refresh = p_new_refresh_id
    WHERE token_hash = digest(p_old_token, 'sha256')
      AND revoked_at IS NULL;
END;
$$ LANGUAGE plpgsql;