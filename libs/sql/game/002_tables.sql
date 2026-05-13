CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Core character record, one per account/name pair.
CREATE TABLE IF NOT EXISTS game.characters (
    character_id uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id   uuid        NOT NULL REFERENCES auth.accounts(account_id) ON DELETE CASCADE,
    name         text        NOT NULL,
    cash         numeric(18,6) NOT NULL DEFAULT 0,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (account_id, name)
);

-- Per-character skill XP. One row per (character, skill).
CREATE TABLE IF NOT EXISTS game.character_skills (
    character_id uuid   NOT NULL REFERENCES game.characters(character_id) ON DELETE CASCADE,
    skill_id     text   NOT NULL,
    xp           bigint NOT NULL DEFAULT 0 CHECK (xp >= 0),
    updated_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (character_id, skill_id)
);

-- Container instances owned by a character.
--
-- Every storage context is a row here: base inventory, equipped backpack,
-- bank, chests, etc. Root containers (inventory, bank) have parent fields
-- NULL. Portable containers (backpacks) point to the parent container and
-- slot they occupy.
CREATE TABLE IF NOT EXISTS game.character_containers (
    container_id        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    character_id        uuid NOT NULL REFERENCES game.characters(character_id) ON DELETE CASCADE,
    kind                text NOT NULL,
    container_def_id    text NOT NULL,
    display_name        text NOT NULL,
    slot_capacity       int  NOT NULL CHECK (slot_capacity >= 0),
    equipped_item_id    text NULL,
    parent_container_id uuid NULL REFERENCES game.character_containers(container_id) ON DELETE SET NULL,
    parent_slot_idx     int  NULL CHECK (parent_slot_idx >= 0),
    created_at          timestamptz NOT NULL DEFAULT now(),
    updated_at          timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT character_containers_parent_slot_pair CHECK (
        (parent_container_id IS NULL AND parent_slot_idx IS NULL)
        OR
        (parent_container_id IS NOT NULL AND parent_slot_idx IS NOT NULL)
    )
);

-- One unique base inventory container per character.
CREATE UNIQUE INDEX IF NOT EXISTS ux_character_containers_base_inventory
    ON game.character_containers (character_id)
    WHERE kind = 'inventory' AND parent_container_id IS NULL;

-- One unique bag slot 0 and bag slot 1 per character.
CREATE UNIQUE INDEX IF NOT EXISTS ux_character_containers_bag_slot
    ON game.character_containers (character_id, container_def_id)
    WHERE kind = 'bag_slot';

-- Positional slot contents for any container instance.
--
-- Covers base inventory, backpack contents, bank tabs, chests, etc.
-- child_container_id links a slot to a nested container (e.g. a backpack
-- item that owns its own container instance).
CREATE TABLE IF NOT EXISTS game.character_container_slots (
    container_id       uuid   NOT NULL REFERENCES game.character_containers(container_id) ON DELETE CASCADE,
    slot_idx           int    NOT NULL CHECK (slot_idx >= 0),
    item_id            text   NOT NULL,
    quantity           bigint NOT NULL CHECK (quantity > 0),
    child_container_id uuid   NULL REFERENCES game.character_containers(container_id) ON DELETE SET NULL,
    created_at         timestamptz NOT NULL DEFAULT now(),
    updated_at         timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (container_id, slot_idx),

    CONSTRAINT character_container_slots_no_self_child CHECK (
        child_container_id IS NULL OR child_container_id <> container_id
    )
);
