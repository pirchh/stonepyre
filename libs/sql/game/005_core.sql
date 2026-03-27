CREATE OR REPLACE FUNCTION game.enforce_max_characters()
RETURNS TRIGGER AS $$
BEGIN
    IF (SELECT COUNT(*) FROM game.characters WHERE account_id = NEW.account_id) >= 5 THEN
        RAISE EXCEPTION 'Account % already has max 5 characters', NEW.account_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_game_enforce_max_characters ON game.characters;
CREATE TRIGGER trg_game_enforce_max_characters
BEFORE INSERT ON game.characters
FOR EACH ROW
EXECUTE FUNCTION game.enforce_max_characters();

CREATE OR REPLACE FUNCTION game.account_owns_character(p_account_id UUID, p_character_id UUID)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM game.characters c
        WHERE c.character_id = p_character_id
          AND c.account_id = p_account_id
    );
END;
$$ LANGUAGE plpgsql;