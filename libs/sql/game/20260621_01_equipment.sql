-- Worn equipment slots (helm, chest, main-hand, …). One row per (character, slot).
--
-- Distinct from game.character_containers, which models inventory/bank/bag
-- containers. Equipment holds a single item per named slot (EquipSlot). The
-- main-hand slot is what gates tool-based harvesting (e.g. an equipped axe).
CREATE TABLE IF NOT EXISTS game.character_equipment (
    character_id uuid        NOT NULL REFERENCES game.characters(character_id) ON DELETE CASCADE,
    slot         text        NOT NULL,
    item_id      text        NOT NULL,
    updated_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (character_id, slot)
);
