-- Add is_admin flag to accounts table.
ALTER TABLE auth.accounts
    ADD COLUMN IF NOT EXISTS is_admin BOOLEAN NOT NULL DEFAULT FALSE;
