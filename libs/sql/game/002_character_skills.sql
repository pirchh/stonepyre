-- Stonepyre Phase 7k-b
-- DB-backed character skill XP.
--
-- Run this manually against your Stonepyre Postgres database before starting
-- the 7k-b server build.
--
-- This matches the existing game.characters schema where the primary character
-- identifier column is character_id, not id.

CREATE TABLE IF NOT EXISTS game.character_skills (
    character_id UUID NOT NULL REFERENCES game.characters(character_id) ON DELETE CASCADE,
    skill_id TEXT NOT NULL,
    xp BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (character_id, skill_id),
    CHECK (xp >= 0)
);

CREATE INDEX IF NOT EXISTS idx_character_skills_character_id
    ON game.character_skills (character_id);

CREATE INDEX IF NOT EXISTS idx_character_skills_skill_id
    ON game.character_skills (skill_id);
