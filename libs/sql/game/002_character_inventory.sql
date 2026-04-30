CREATE TABLE IF NOT EXISTS game.character_inventory (
    character_id uuid NOT NULL REFERENCES game.characters(character_id) ON DELETE CASCADE,
    item_id text NOT NULL,
    quantity bigint NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT character_inventory_quantity_nonnegative CHECK (quantity >= 0),
    CONSTRAINT character_inventory_pkey PRIMARY KEY (character_id, item_id)
);

CREATE INDEX IF NOT EXISTS character_inventory_character_id_idx
    ON game.character_inventory (character_id);
