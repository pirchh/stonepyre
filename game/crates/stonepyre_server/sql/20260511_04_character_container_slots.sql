-- Stonepyre server schema: slots for character-owned container instances
--
-- This is the future-facing positional slot table. Every container instance can
-- own slots here: base inventory, equipped backpack contents, bank tabs, chests,
-- etc.
--
-- The current branch keeps game.character_inventory_slots as a compatibility
-- bridge for the base inventory. A later branch can migrate base inventory fully
-- onto character_containers + character_container_slots once the app/server are
-- ready to resolve container instance ids everywhere.

CREATE TABLE IF NOT EXISTS game.character_container_slots (
    container_id uuid NOT NULL REFERENCES game.character_containers(id) ON DELETE CASCADE,
    slot_idx int NOT NULL CHECK (slot_idx >= 0),
    item_id text NOT NULL,
    quantity bigint NOT NULL CHECK (quantity > 0),

    -- If the item in this slot owns a nested container, point to that container
    -- instance. Example: backpack item in inventory slot -> backpack contents.
    child_container_id uuid NULL REFERENCES game.character_containers(id) ON DELETE SET NULL,

    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (container_id, slot_idx),

    CONSTRAINT character_container_slots_no_self_child CHECK (
        child_container_id IS NULL OR child_container_id <> container_id
    )
);

CREATE INDEX IF NOT EXISTS idx_character_container_slots_item
    ON game.character_container_slots (item_id);

CREATE INDEX IF NOT EXISTS idx_character_container_slots_child_container
    ON game.character_container_slots (child_container_id);
