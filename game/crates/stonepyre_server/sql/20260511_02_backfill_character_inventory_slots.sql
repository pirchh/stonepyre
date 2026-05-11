-- Stonepyre server data migration: backfill slot-backed player inventories
--
-- This expands existing aggregate rows from game.character_inventory into
-- game.character_inventory_slots for characters that do not already have slot
-- rows. Non-stackable/unknown aggregate rows are expanded as one item per slot;
-- this matches the current OSRS-style inventory behavior for logs.
--
-- IMPORTANT:
--   This is a compatibility backfill for the current content set. Once item
--   definitions live in SQL or migrations know stack policy directly, this can
--   be made item-policy aware. Today, known player-inventory stackables can be
--   added to the stackable_items CTE below.

WITH stackable_items(item_id) AS (
    VALUES
        -- No known stack-in-inventory items yet.
        ('__none__')
), characters_without_slots AS (
    SELECT DISTINCT ci.character_id
    FROM game.character_inventory ci
    WHERE ci.quantity > 0
      AND NOT EXISTS (
          SELECT 1
          FROM game.character_inventory_slots cis
          WHERE cis.character_id = ci.character_id
            AND cis.container_id = 'inventory'
      )
), expanded AS (
    SELECT
        ci.character_id,
        ci.item_id,
        CASE
            WHEN si.item_id IS NOT NULL THEN ci.quantity
            ELSE 1
        END AS quantity,
        row_number() OVER (
            PARTITION BY ci.character_id
            ORDER BY ci.updated_at ASC, ci.item_id ASC, gs.n ASC
        ) - 1 AS slot_idx
    FROM game.character_inventory ci
    JOIN characters_without_slots cws
      ON cws.character_id = ci.character_id
    LEFT JOIN stackable_items si
      ON si.item_id = ci.item_id
    JOIN LATERAL generate_series(
        1,
        CASE
            WHEN si.item_id IS NOT NULL THEN 1
            ELSE LEAST(ci.quantity, 20)
        END
    ) AS gs(n) ON TRUE
    WHERE ci.quantity > 0
)
INSERT INTO game.character_inventory_slots (
    character_id,
    container_id,
    slot_idx,
    item_id,
    quantity
)
SELECT
    character_id,
    'inventory',
    slot_idx::int,
    item_id,
    quantity
FROM expanded
WHERE slot_idx < 20
ON CONFLICT (character_id, container_id, slot_idx) DO NOTHING;
