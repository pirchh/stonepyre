ALTER TABLE game.character_containers
    ADD COLUMN IF NOT EXISTS equipped_item_id text NULL;

CREATE UNIQUE INDEX IF NOT EXISTS ux_character_containers_bag_slot
    ON game.character_containers (character_id, container_def_id)
    WHERE kind = 'bag_slot';
