CREATE INDEX IF NOT EXISTS idx_game_characters_account
    ON game.characters (account_id);

CREATE INDEX IF NOT EXISTS idx_character_skills_character_id
    ON game.character_skills (character_id);

CREATE INDEX IF NOT EXISTS idx_character_skills_skill_id
    ON game.character_skills (skill_id);

CREATE INDEX IF NOT EXISTS idx_character_containers_character
    ON game.character_containers (character_id);

CREATE INDEX IF NOT EXISTS idx_character_containers_parent
    ON game.character_containers (parent_container_id, parent_slot_idx);

CREATE INDEX IF NOT EXISTS idx_character_container_slots_item
    ON game.character_container_slots (item_id);

CREATE INDEX IF NOT EXISTS idx_character_container_slots_child_container
    ON game.character_container_slots (child_container_id);
