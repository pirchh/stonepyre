--
-- PostgreSQL database dump
--

\restrict hlJIbbiNnTa6xPHy7rjVJ4e1hVYDuzvENh67NLDxd4N70bQEHkSkDQiajYZzasN

-- Dumped from database version 17.6
-- Dumped by pg_dump version 17.6

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: game; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA game;


--
-- Name: account_owns_character(uuid, uuid); Type: FUNCTION; Schema: game; Owner: -
--

CREATE FUNCTION game.account_owns_character(p_account_id uuid, p_character_id uuid) RETURNS boolean
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM game.characters c
        WHERE c.character_id = p_character_id
          AND c.account_id = p_account_id
    );
END;
$$;


--
-- Name: enforce_max_characters(); Type: FUNCTION; Schema: game; Owner: -
--

CREATE FUNCTION game.enforce_max_characters() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (SELECT COUNT(*) FROM game.characters WHERE account_id = NEW.account_id) >= 5 THEN
        RAISE EXCEPTION 'Account % already has max 5 characters', NEW.account_id;
    END IF;
    RETURN NEW;
END;
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: characters; Type: TABLE; Schema: game; Owner: -
--

CREATE TABLE game.characters (
    character_id uuid DEFAULT gen_random_uuid() NOT NULL,
    account_id uuid NOT NULL,
    name text NOT NULL,
    cash numeric(18,6) DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: characters characters_account_id_name_key; Type: CONSTRAINT; Schema: game; Owner: -
--

ALTER TABLE ONLY game.characters
    ADD CONSTRAINT characters_account_id_name_key UNIQUE (account_id, name);


--
-- Name: characters characters_pkey; Type: CONSTRAINT; Schema: game; Owner: -
--

ALTER TABLE ONLY game.characters
    ADD CONSTRAINT characters_pkey PRIMARY KEY (character_id);


--
-- Name: idx_game_characters_account; Type: INDEX; Schema: game; Owner: -
--

CREATE INDEX idx_game_characters_account ON game.characters USING btree (account_id);


--
-- Name: characters trg_game_enforce_max_characters; Type: TRIGGER; Schema: game; Owner: -
--

CREATE TRIGGER trg_game_enforce_max_characters BEFORE INSERT ON game.characters FOR EACH ROW EXECUTE FUNCTION game.enforce_max_characters();


--
-- Name: characters characters_account_id_fkey; Type: FK CONSTRAINT; Schema: game; Owner: -
--

ALTER TABLE ONLY game.characters
    ADD CONSTRAINT characters_account_id_fkey FOREIGN KEY (account_id) REFERENCES auth.accounts(account_id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

\unrestrict hlJIbbiNnTa6xPHy7rjVJ4e1hVYDuzvENh67NLDxd4N70bQEHkSkDQiajYZzasN

