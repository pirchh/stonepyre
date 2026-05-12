-- Stonepyre game schema: slot-backed character inventory
--
-- This table is the server-authoritative positional inventory store.
-- The existing game.character_inventory aggregate table is kept in sync as a
-- compatibility/read-model bridge while older code paths are retired.
--
-- Container scope:
--   container_id = 'inventory' is the player's base inventory for this pass.
--   Later passes can introduce container instance ids for backpacks, banks,
--   chests, etc. without changing the slot row shape.

CREATE TABLE IF NOT EXISTS game.character_inventory_slots (
    character_id uuid NOT NULL REFERENCES game.characters(character_id) ON DELETE CASCADE,
    container_id text NOT NULL DEFAULT 'inventory',
    slot_idx int NOT NULL,
    item_id text NOT NULL,
    quantity bigint NOT NULL CHECK (quantity > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (character_id, container_id, slot_idx)
);

CREATE INDEX IF NOT EXISTS idx_character_inventory_slots_character_container
    ON game.character_inventory_slots (character_id, container_id);

CREATE INDEX IF NOT EXISTS idx_character_inventory_slots_character_item
    ON game.character_inventory_slots (character_id, item_id);
