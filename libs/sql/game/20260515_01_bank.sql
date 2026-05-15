-- Bank system: add tab_idx and tag_filters to character_containers.
--
-- Bank tabs are rows in character_containers with kind = 'bank_tab'.
-- tab_idx 0 is the implicit "All" view (never stored). Physical tabs start at 1.
-- tag_filters stores the item tags that auto-route deposits into this tab.
-- Items not matching any tab filter go to the lowest-indexed tab (usually tab 1,
-- the default "General" tab every character starts with).

ALTER TABLE game.character_containers
    ADD COLUMN IF NOT EXISTS tab_idx      int       NULL,
    ADD COLUMN IF NOT EXISTS tag_filters  text[]    NULL;

-- Each bank tab is uniquely identified by (character_id, tab_idx).
CREATE UNIQUE INDEX IF NOT EXISTS ux_character_containers_bank_tab
    ON game.character_containers (character_id, tab_idx)
    WHERE kind = 'bank_tab';
