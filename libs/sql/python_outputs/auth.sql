--
-- PostgreSQL database dump
--

\restrict LcAW61kYSdQRHzodNyJxsfI3cLTxTc3E8TxpnRwTeC2p1PJXzTwnmXeczMIaip2

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
-- Name: auth; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA auth;


--
-- Name: hash_password(text, integer); Type: FUNCTION; Schema: auth; Owner: -
--

CREATE FUNCTION auth.hash_password(p_password text, p_cost integer DEFAULT 12) RETURNS text
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN crypt(p_password, gen_salt('bf', p_cost));
END;
$$;


--
-- Name: login(text, text); Type: FUNCTION; Schema: auth; Owner: -
--

CREATE FUNCTION auth.login(p_email text, p_password text) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: new_refresh(uuid, text, integer, text, inet); Type: FUNCTION; Schema: auth; Owner: -
--

CREATE FUNCTION auth.new_refresh(p_account_id uuid, p_token text, p_ttl_seconds integer, p_user_agent text DEFAULT NULL::text, p_ip inet DEFAULT NULL::inet) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: new_session(uuid, text, integer, text, inet); Type: FUNCTION; Schema: auth; Owner: -
--

CREATE FUNCTION auth.new_session(p_account_id uuid, p_token text, p_ttl_seconds integer, p_user_agent text DEFAULT NULL::text, p_ip inet DEFAULT NULL::inet) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: revoke_session(text); Type: FUNCTION; Schema: auth; Owner: -
--

CREATE FUNCTION auth.revoke_session(p_token text) RETURNS void
    LANGUAGE plpgsql
    AS $$
BEGIN
    UPDATE auth.sessions
    SET revoked_at = NOW()
    WHERE token_hash = digest(p_token, 'sha256')
      AND revoked_at IS NULL;
END;
$$;


--
-- Name: rotate_refresh(text, uuid); Type: FUNCTION; Schema: auth; Owner: -
--

CREATE FUNCTION auth.rotate_refresh(p_old_token text, p_new_refresh_id uuid) RETURNS void
    LANGUAGE plpgsql
    AS $$
BEGIN
    UPDATE auth.refresh_tokens
    SET revoked_at = NOW(),
        replaced_by_refresh = p_new_refresh_id
    WHERE token_hash = digest(p_old_token, 'sha256')
      AND revoked_at IS NULL;
END;
$$;


--
-- Name: touch_updated_at(); Type: FUNCTION; Schema: auth; Owner: -
--

CREATE FUNCTION auth.touch_updated_at() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


--
-- Name: verify_password(text, text); Type: FUNCTION; Schema: auth; Owner: -
--

CREATE FUNCTION auth.verify_password(p_password text, p_hash text) RETURNS boolean
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN (p_hash = crypt(p_password, p_hash));
END;
$$;


--
-- Name: verify_refresh(text); Type: FUNCTION; Schema: auth; Owner: -
--

CREATE FUNCTION auth.verify_refresh(p_token text) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: verify_session(text); Type: FUNCTION; Schema: auth; Owner: -
--

CREATE FUNCTION auth.verify_session(p_token text) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
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
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: account_roles; Type: TABLE; Schema: auth; Owner: -
--

CREATE TABLE auth.account_roles (
    account_id uuid NOT NULL,
    role_id integer NOT NULL,
    granted_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: accounts; Type: TABLE; Schema: auth; Owner: -
--

CREATE TABLE auth.accounts (
    account_id uuid DEFAULT gen_random_uuid() NOT NULL,
    email text NOT NULL,
    display_name text,
    password_hash text NOT NULL,
    is_disabled boolean DEFAULT false NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL,
    last_login_at timestamp without time zone
);


--
-- Name: refresh_tokens; Type: TABLE; Schema: auth; Owner: -
--

CREATE TABLE auth.refresh_tokens (
    refresh_id uuid DEFAULT gen_random_uuid() NOT NULL,
    account_id uuid NOT NULL,
    token_hash bytea NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    revoked_at timestamp without time zone,
    replaced_by_refresh uuid,
    ip_addr inet,
    user_agent text
);


--
-- Name: roles; Type: TABLE; Schema: auth; Owner: -
--

CREATE TABLE auth.roles (
    role_id smallint NOT NULL,
    role_name text NOT NULL
);


--
-- Name: roles_role_id_seq; Type: SEQUENCE; Schema: auth; Owner: -
--

CREATE SEQUENCE auth.roles_role_id_seq
    AS smallint
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: roles_role_id_seq; Type: SEQUENCE OWNED BY; Schema: auth; Owner: -
--

ALTER SEQUENCE auth.roles_role_id_seq OWNED BY auth.roles.role_id;


--
-- Name: sessions; Type: TABLE; Schema: auth; Owner: -
--

CREATE TABLE auth.sessions (
    session_id uuid DEFAULT gen_random_uuid() NOT NULL,
    account_id uuid NOT NULL,
    token_hash bytea NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    revoked_at timestamp without time zone,
    ip_addr inet,
    user_agent text
);


--
-- Name: roles role_id; Type: DEFAULT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.roles ALTER COLUMN role_id SET DEFAULT nextval('auth.roles_role_id_seq'::regclass);


--
-- Name: account_roles account_roles_pkey; Type: CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.account_roles
    ADD CONSTRAINT account_roles_pkey PRIMARY KEY (account_id, role_id);


--
-- Name: accounts accounts_email_key; Type: CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.accounts
    ADD CONSTRAINT accounts_email_key UNIQUE (email);


--
-- Name: accounts accounts_pkey; Type: CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.accounts
    ADD CONSTRAINT accounts_pkey PRIMARY KEY (account_id);


--
-- Name: refresh_tokens refresh_tokens_pkey; Type: CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.refresh_tokens
    ADD CONSTRAINT refresh_tokens_pkey PRIMARY KEY (refresh_id);


--
-- Name: refresh_tokens refresh_tokens_token_hash_key; Type: CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.refresh_tokens
    ADD CONSTRAINT refresh_tokens_token_hash_key UNIQUE (token_hash);


--
-- Name: roles roles_pkey; Type: CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.roles
    ADD CONSTRAINT roles_pkey PRIMARY KEY (role_id);


--
-- Name: roles roles_role_name_key; Type: CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.roles
    ADD CONSTRAINT roles_role_name_key UNIQUE (role_name);


--
-- Name: sessions sessions_pkey; Type: CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.sessions
    ADD CONSTRAINT sessions_pkey PRIMARY KEY (session_id);


--
-- Name: sessions sessions_token_hash_key; Type: CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.sessions
    ADD CONSTRAINT sessions_token_hash_key UNIQUE (token_hash);


--
-- Name: idx_auth_account_roles_account; Type: INDEX; Schema: auth; Owner: -
--

CREATE INDEX idx_auth_account_roles_account ON auth.account_roles USING btree (account_id);


--
-- Name: idx_auth_refresh_account; Type: INDEX; Schema: auth; Owner: -
--

CREATE INDEX idx_auth_refresh_account ON auth.refresh_tokens USING btree (account_id);


--
-- Name: idx_auth_refresh_expires; Type: INDEX; Schema: auth; Owner: -
--

CREATE INDEX idx_auth_refresh_expires ON auth.refresh_tokens USING btree (expires_at);


--
-- Name: idx_auth_sessions_account; Type: INDEX; Schema: auth; Owner: -
--

CREATE INDEX idx_auth_sessions_account ON auth.sessions USING btree (account_id);


--
-- Name: idx_auth_sessions_expires; Type: INDEX; Schema: auth; Owner: -
--

CREATE INDEX idx_auth_sessions_expires ON auth.sessions USING btree (expires_at);


--
-- Name: accounts trg_auth_accounts_touch; Type: TRIGGER; Schema: auth; Owner: -
--

CREATE TRIGGER trg_auth_accounts_touch BEFORE UPDATE ON auth.accounts FOR EACH ROW EXECUTE FUNCTION auth.touch_updated_at();


--
-- Name: account_roles account_roles_account_id_fkey; Type: FK CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.account_roles
    ADD CONSTRAINT account_roles_account_id_fkey FOREIGN KEY (account_id) REFERENCES auth.accounts(account_id) ON DELETE CASCADE;


--
-- Name: account_roles account_roles_role_id_fkey; Type: FK CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.account_roles
    ADD CONSTRAINT account_roles_role_id_fkey FOREIGN KEY (role_id) REFERENCES auth.roles(role_id) ON DELETE CASCADE;


--
-- Name: refresh_tokens refresh_tokens_account_id_fkey; Type: FK CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.refresh_tokens
    ADD CONSTRAINT refresh_tokens_account_id_fkey FOREIGN KEY (account_id) REFERENCES auth.accounts(account_id) ON DELETE CASCADE;


--
-- Name: refresh_tokens refresh_tokens_replaced_by_refresh_fkey; Type: FK CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.refresh_tokens
    ADD CONSTRAINT refresh_tokens_replaced_by_refresh_fkey FOREIGN KEY (replaced_by_refresh) REFERENCES auth.refresh_tokens(refresh_id);


--
-- Name: sessions sessions_account_id_fkey; Type: FK CONSTRAINT; Schema: auth; Owner: -
--

ALTER TABLE ONLY auth.sessions
    ADD CONSTRAINT sessions_account_id_fkey FOREIGN KEY (account_id) REFERENCES auth.accounts(account_id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

\unrestrict LcAW61kYSdQRHzodNyJxsfI3cLTxTc3E8TxpnRwTeC2p1PJXzTwnmXeczMIaip2

