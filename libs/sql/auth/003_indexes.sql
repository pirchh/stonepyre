CREATE INDEX IF NOT EXISTS idx_auth_sessions_account
ON auth.sessions(account_id);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_expires
ON auth.sessions(expires_at);

CREATE INDEX IF NOT EXISTS idx_auth_refresh_account
ON auth.refresh_tokens(account_id);

CREATE INDEX IF NOT EXISTS idx_auth_refresh_expires
ON auth.refresh_tokens(expires_at);

CREATE INDEX IF NOT EXISTS idx_auth_account_roles_account
ON auth.account_roles(account_id);