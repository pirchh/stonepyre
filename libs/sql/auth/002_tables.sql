CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ==============================
-- Accounts
-- ==============================
CREATE TABLE IF NOT EXISTS auth.accounts (
    account_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email            TEXT UNIQUE NOT NULL,
    display_name     TEXT NULL,

    password_hash    TEXT NOT NULL,              -- bcrypt (crypt)
    is_disabled      BOOLEAN NOT NULL DEFAULT FALSE,

    created_at       TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP NOT NULL DEFAULT NOW(),
    last_login_at    TIMESTAMP NULL
);

-- ==============================
-- Roles (RBAC)
-- ==============================
CREATE TABLE IF NOT EXISTS auth.roles (
    role_id      SMALLSERIAL PRIMARY KEY,
    role_name    TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS auth.account_roles (
    account_id   UUID NOT NULL REFERENCES auth.accounts(account_id) ON DELETE CASCADE,
    role_id      INT NOT NULL REFERENCES auth.roles(role_id) ON DELETE CASCADE,
    granted_at   TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, role_id)
);

-- Seed roles: 'player', 'admin' etc. done in core.sql

-- ==============================
-- Sessions (access tokens)
-- ==============================
CREATE TABLE IF NOT EXISTS auth.sessions (
    session_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id       UUID NOT NULL REFERENCES auth.accounts(account_id) ON DELETE CASCADE,

    -- access token hash: digest(token, 'sha256')
    token_hash       BYTEA NOT NULL UNIQUE,

    created_at       TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at       TIMESTAMP NOT NULL,
    revoked_at       TIMESTAMP NULL,

    -- telemetry / abuse controls
    ip_addr          INET NULL,
    user_agent       TEXT NULL
);

-- ==============================
-- Refresh tokens (rotation)
-- ==============================
CREATE TABLE IF NOT EXISTS auth.refresh_tokens (
    refresh_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id            UUID NOT NULL REFERENCES auth.accounts(account_id) ON DELETE CASCADE,

    -- refresh token hash: digest(token, 'sha256')
    token_hash            BYTEA NOT NULL UNIQUE,

    created_at            TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at            TIMESTAMP NOT NULL,
    revoked_at            TIMESTAMP NULL,

    -- rotation support: if this refresh is used, you mint a new refresh
    replaced_by_refresh   UUID NULL REFERENCES auth.refresh_tokens(refresh_id),

    ip_addr               INET NULL,
    user_agent            TEXT NULL
);