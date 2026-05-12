-- Stonepyre game schema: character-owned container instances
--
-- This is the future-facing container layer for backpacks, banks, chests, and
-- nested inventories. The current branch still uses game.character_inventory_slots
-- as a compatibility bridge for the base player inventory, but this table gives
-- us the proper place to represent containers as first-class owned objects.
--
-- Examples:
--   base inventory: kind='inventory', container_def_id='base_inventory'
--   backpack item: kind='backpack', container_def_id='wooden_backpack'
--   bank:          kind='bank',      container_def_id='bank'
--
-- parent_container_id + parent_slot_idx identify where a portable/nested
-- container lives. For the base inventory and bank, parent fields are NULL.

CREATE TABLE IF NOT EXISTS game.character_containers (
    container_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    character_id uuid NOT NULL REFERENCES game.characters(character_id) ON DELETE CASCADE,
    kind text NOT NULL,
    container_def_id text NOT NULL,
    display_name text NOT NULL,
    slot_capacity int NOT NULL CHECK (slot_capacity >= 0),
    parent_container_id uuid NULL REFERENCES game.character_containers(container_id) ON DELETE SET NULL,
    parent_slot_idx int NULL CHECK (parent_slot_idx >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT character_containers_parent_slot_pair CHECK (
        (parent_container_id IS NULL AND parent_slot_idx IS NULL)
        OR
        (parent_container_id IS NOT NULL AND parent_slot_idx IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_character_containers_character
    ON game.character_containers (character_id);

CREATE INDEX IF NOT EXISTS idx_character_containers_parent
    ON game.character_containers (parent_container_id, parent_slot_idx);

CREATE UNIQUE INDEX IF NOT EXISTS ux_character_containers_base_inventory
    ON game.character_containers (character_id)
    WHERE kind = 'inventory' AND parent_container_id IS NULL;
