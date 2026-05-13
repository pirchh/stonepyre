-- Stonepyre game schema: migrate base inventory onto container-instance model
--
-- Creates a character_containers row (kind='inventory') for every character,
-- then copies their character_inventory_slots rows into character_container_slots
-- keyed by that container's uuid.
--
-- After this migration, character_inventory_slots is no longer the
-- authoritative slot store. The server will read/write character_container_slots
-- exclusively for base inventory operations.

-- Step 1: ensure every character has a base inventory container instance.
INSERT INTO game.character_containers (
    character_id,
    kind,
    container_def_id,
    display_name,
    slot_capacity
)
SELECT
    c.character_id,
    'inventory',
    'base_inventory',
    'Inventory',
    20
FROM game.characters c
WHERE NOT EXISTS (
    SELECT 1
    FROM game.character_containers cc
    WHERE cc.character_id = c.character_id
      AND cc.kind = 'inventory'
      AND cc.parent_container_id IS NULL
);

-- Step 2: copy existing slot rows from character_inventory_slots into
-- character_container_slots, keyed by the container uuid created above.
INSERT INTO game.character_container_slots (
    container_id,
    slot_idx,
    item_id,
    quantity
)
SELECT
    cc.container_id,
    cis.slot_idx,
    cis.item_id,
    cis.quantity
FROM game.character_inventory_slots cis
JOIN game.character_containers cc
    ON cc.character_id = cis.character_id
   AND cc.kind = 'inventory'
   AND cc.parent_container_id IS NULL
WHERE cis.container_id = 'inventory'
ON CONFLICT (container_id, slot_idx) DO NOTHING;
