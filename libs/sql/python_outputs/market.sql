--
-- PostgreSQL database dump
--

\restrict 9a24LCvsn9VBek5d5kkilbXWa5ZzKOOTzBTfVPH8lPO5kx1PgD6T3eRVv2uPH3Z

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
-- Name: market; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA market;


--
-- Name: company_status; Type: TYPE; Schema: market; Owner: -
--

CREATE TYPE market.company_status AS ENUM (
    'ACTIVE',
    'DELISTED',
    'BANKRUPT'
);


--
-- Name: company_trend; Type: TYPE; Schema: market; Owner: -
--

CREATE TYPE market.company_trend AS ENUM (
    'NORMAL',
    'DECLINING',
    'BOOMING'
);


--
-- Name: season; Type: TYPE; Schema: market; Owner: -
--

CREATE TYPE market.season AS ENUM (
    'SPRING',
    'SUMMER',
    'FALL',
    'WINTER'
);


--
-- Name: get_clock(); Type: FUNCTION; Schema: market; Owner: -
--

CREATE FUNCTION market.get_clock() RETURNS TABLE(sim_day integer, minute_of_day integer, is_open boolean)
    LANGUAGE sql
    AS $$
  SELECT sim_day, minute_of_day, is_open
  FROM market.clock
  WHERE id = 1
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: candles_1h; Type: TABLE; Schema: market; Owner: -
--

CREATE TABLE market.candles_1h (
    company_id bigint NOT NULL,
    sim_day bigint NOT NULL,
    opened_at timestamp without time zone NOT NULL,
    closed_at timestamp without time zone NOT NULL,
    open double precision NOT NULL,
    high double precision NOT NULL,
    low double precision NOT NULL,
    close double precision NOT NULL,
    volume bigint DEFAULT 0 NOT NULL
);


--
-- Name: clock; Type: TABLE; Schema: market; Owner: -
--

CREATE TABLE market.clock (
    id integer NOT NULL,
    started_at timestamp without time zone DEFAULT now() NOT NULL,
    last_advance_at timestamp without time zone DEFAULT now() NOT NULL,
    sim_day integer DEFAULT 0 NOT NULL,
    minute_of_day integer DEFAULT 0 NOT NULL,
    day_length_minutes integer DEFAULT 60 NOT NULL,
    market_close_minutes integer DEFAULT 15 NOT NULL,
    days_per_year integer DEFAULT 360 NOT NULL,
    season_length_days integer DEFAULT 90 NOT NULL,
    season market.season DEFAULT 'SPRING'::market.season NOT NULL,
    is_open boolean DEFAULT true NOT NULL,
    CONSTRAINT clock_id_check CHECK ((id = 1))
);


--
-- Name: companies; Type: TABLE; Schema: market; Owner: -
--

CREATE TABLE market.companies (
    company_id bigint NOT NULL,
    name text NOT NULL,
    industry_id integer NOT NULL,
    status market.company_status DEFAULT 'ACTIVE'::market.company_status NOT NULL,
    listed boolean DEFAULT true NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    delisted_at timestamp without time zone,
    bankrupt_at timestamp without time zone,
    revived_at timestamp without time zone,
    revival_count integer DEFAULT 0 NOT NULL,
    base_volatility double precision DEFAULT 0.05 NOT NULL,
    quality_score double precision DEFAULT 1.0 NOT NULL
);


--
-- Name: companies_company_id_seq; Type: SEQUENCE; Schema: market; Owner: -
--

CREATE SEQUENCE market.companies_company_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: companies_company_id_seq; Type: SEQUENCE OWNED BY; Schema: market; Owner: -
--

ALTER SEQUENCE market.companies_company_id_seq OWNED BY market.companies.company_id;


--
-- Name: events; Type: TABLE; Schema: market; Owner: -
--

CREATE TABLE market.events (
    event_id bigint NOT NULL,
    company_id bigint NOT NULL,
    ts timestamp without time zone DEFAULT now() NOT NULL,
    event_type text NOT NULL,
    payload jsonb
);


--
-- Name: events_event_id_seq; Type: SEQUENCE; Schema: market; Owner: -
--

CREATE SEQUENCE market.events_event_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: events_event_id_seq; Type: SEQUENCE OWNED BY; Schema: market; Owner: -
--

ALTER SEQUENCE market.events_event_id_seq OWNED BY market.events.event_id;


--
-- Name: industries; Type: TABLE; Schema: market; Owner: -
--

CREATE TABLE market.industries (
    industry_id integer NOT NULL,
    code text NOT NULL,
    name text NOT NULL,
    cap integer,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    weight numeric(10,4) DEFAULT 1.0 NOT NULL
);


--
-- Name: industries_industry_id_seq; Type: SEQUENCE; Schema: market; Owner: -
--

CREATE SEQUENCE market.industries_industry_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: industries_industry_id_seq; Type: SEQUENCE OWNED BY; Schema: market; Owner: -
--

ALTER SEQUENCE market.industries_industry_id_seq OWNED BY market.industries.industry_id;


--
-- Name: industry_seasonality; Type: TABLE; Schema: market; Owner: -
--

CREATE TABLE market.industry_seasonality (
    industry_id integer NOT NULL,
    season market.season NOT NULL,
    drift_mult numeric(10,4) DEFAULT 1.0 NOT NULL,
    vol_mult numeric(10,4) DEFAULT 1.0 NOT NULL,
    volume_mult numeric(10,4) DEFAULT 1.0 NOT NULL,
    bankrupt_mult numeric(10,4) DEFAULT 1.0 NOT NULL,
    spawn_mult numeric(10,4) DEFAULT 1.0 NOT NULL
);


--
-- Name: market_state; Type: TABLE; Schema: market; Owner: -
--

CREATE TABLE market.market_state (
    id integer NOT NULL,
    tick bigint DEFAULT 0 NOT NULL,
    last_tick_at timestamp without time zone DEFAULT now() NOT NULL,
    sim_seed bigint NOT NULL,
    sim_version text NOT NULL,
    sim_day bigint DEFAULT 0 NOT NULL,
    day_opened_at timestamp without time zone,
    day_closed_at timestamp without time zone,
    market_is_open boolean DEFAULT true NOT NULL,
    last_advance_at timestamp without time zone DEFAULT now() NOT NULL,
    day_length_minutes integer DEFAULT 60 NOT NULL,
    market_close_minutes integer DEFAULT 15 NOT NULL,
    CONSTRAINT market_state_id_check CHECK ((id = 1))
);


--
-- Name: prices; Type: TABLE; Schema: market; Owner: -
--

CREATE TABLE market.prices (
    company_id bigint NOT NULL,
    ts timestamp without time zone NOT NULL,
    price double precision NOT NULL,
    volume bigint DEFAULT 0 NOT NULL
);


--
-- Name: prices_ticks; Type: TABLE; Schema: market; Owner: -
--

CREATE TABLE market.prices_ticks (
    company_id bigint NOT NULL,
    sim_day bigint NOT NULL,
    ts timestamp without time zone NOT NULL,
    price double precision NOT NULL,
    volume bigint DEFAULT 0 NOT NULL
);


--
-- Name: v_companies_active; Type: VIEW; Schema: market; Owner: -
--

CREATE VIEW market.v_companies_active AS
 SELECT company_id,
    name,
    industry_id,
    status,
    listed,
    created_at,
    delisted_at,
    bankrupt_at,
    revived_at,
    revival_count,
    base_volatility,
    quality_score
   FROM market.companies
  WHERE ((status = 'ACTIVE'::market.company_status) AND (listed = true));


--
-- Name: v_companies_delisted; Type: VIEW; Schema: market; Owner: -
--

CREATE VIEW market.v_companies_delisted AS
 SELECT company_id,
    name,
    industry_id,
    status,
    listed,
    created_at,
    delisted_at,
    bankrupt_at,
    revived_at,
    revival_count,
    base_volatility,
    quality_score
   FROM market.companies
  WHERE (listed = false);


--
-- Name: v_intraday_ohlc; Type: VIEW; Schema: market; Owner: -
--

CREATE VIEW market.v_intraday_ohlc AS
 SELECT t.company_id,
    t.sim_day,
    min(t.ts) AS first_ts,
    max(t.ts) AS last_ts,
    (array_agg(t.price ORDER BY t.ts))[1] AS open,
    max(t.price) AS high,
    min(t.price) AS low,
    (array_agg(t.price ORDER BY t.ts DESC))[1] AS close,
    sum(t.volume) AS volume
   FROM (market.prices_ticks t
     JOIN market.market_state s ON ((s.id = 1)))
  WHERE (t.sim_day = s.sim_day)
  GROUP BY t.company_id, t.sim_day;


--
-- Name: v_latest_candle; Type: VIEW; Schema: market; Owner: -
--

CREATE VIEW market.v_latest_candle AS
 SELECT DISTINCT ON (company_id) company_id,
    sim_day,
    closed_at AS ts,
    close AS price,
    volume,
    open,
    high,
    low,
    close
   FROM market.candles_1h c
  ORDER BY company_id, sim_day DESC;


--
-- Name: v_latest_tick; Type: VIEW; Schema: market; Owner: -
--

CREATE VIEW market.v_latest_tick AS
 SELECT DISTINCT ON (t.company_id) t.company_id,
    t.sim_day,
    t.ts,
    t.price,
    t.volume
   FROM (market.prices_ticks t
     JOIN market.market_state s ON ((s.id = 1)))
  WHERE (t.sim_day = s.sim_day)
  ORDER BY t.company_id, t.ts DESC;


--
-- Name: v_latest_price; Type: VIEW; Schema: market; Owner: -
--

CREATE VIEW market.v_latest_price AS
 SELECT co.company_id,
    COALESCE(lt.ts, lc.ts) AS ts,
    COALESCE(lt.price, lc.price) AS price,
    COALESCE(lt.volume, lc.volume, (0)::bigint) AS volume
   FROM ((market.companies co
     LEFT JOIN market.v_latest_tick lt ON ((lt.company_id = co.company_id)))
     LEFT JOIN market.v_latest_candle lc ON ((lc.company_id = co.company_id)));


--
-- Name: v_market_board; Type: VIEW; Schema: market; Owner: -
--

CREATE VIEW market.v_market_board AS
 SELECT c.company_id,
    c.name,
    i.name AS industry,
    lp.price,
    lp.ts,
    io.open AS day_open,
    io.high AS day_high,
    io.low AS day_low,
    io.close AS day_close,
    io.volume AS day_volume
   FROM (((market.companies c
     JOIN market.industries i ON ((i.industry_id = c.industry_id)))
     LEFT JOIN market.v_latest_price lp ON ((lp.company_id = c.company_id)))
     LEFT JOIN market.v_intraday_ohlc io ON ((io.company_id = c.company_id)))
  WHERE (c.status = 'ACTIVE'::market.company_status)
  ORDER BY c.company_id;


--
-- Name: v_revival_candidates; Type: VIEW; Schema: market; Owner: -
--

CREATE VIEW market.v_revival_candidates AS
 SELECT company_id,
    industry_id,
    bankrupt_at,
    revival_count
   FROM market.companies c
  WHERE ((status = 'BANKRUPT'::market.company_status) AND (bankrupt_at IS NOT NULL));


--
-- Name: companies company_id; Type: DEFAULT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.companies ALTER COLUMN company_id SET DEFAULT nextval('market.companies_company_id_seq'::regclass);


--
-- Name: events event_id; Type: DEFAULT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.events ALTER COLUMN event_id SET DEFAULT nextval('market.events_event_id_seq'::regclass);


--
-- Name: industries industry_id; Type: DEFAULT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.industries ALTER COLUMN industry_id SET DEFAULT nextval('market.industries_industry_id_seq'::regclass);


--
-- Name: candles_1h candles_1h_pkey; Type: CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.candles_1h
    ADD CONSTRAINT candles_1h_pkey PRIMARY KEY (company_id, sim_day);


--
-- Name: clock clock_pkey; Type: CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.clock
    ADD CONSTRAINT clock_pkey PRIMARY KEY (id);


--
-- Name: companies companies_pkey; Type: CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.companies
    ADD CONSTRAINT companies_pkey PRIMARY KEY (company_id);


--
-- Name: events events_pkey; Type: CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.events
    ADD CONSTRAINT events_pkey PRIMARY KEY (event_id);


--
-- Name: industries industries_code_key; Type: CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.industries
    ADD CONSTRAINT industries_code_key UNIQUE (code);


--
-- Name: industries industries_pkey; Type: CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.industries
    ADD CONSTRAINT industries_pkey PRIMARY KEY (industry_id);


--
-- Name: industry_seasonality industry_seasonality_pkey; Type: CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.industry_seasonality
    ADD CONSTRAINT industry_seasonality_pkey PRIMARY KEY (industry_id, season);


--
-- Name: market_state market_state_pkey; Type: CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.market_state
    ADD CONSTRAINT market_state_pkey PRIMARY KEY (id);


--
-- Name: prices prices_pkey; Type: CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.prices
    ADD CONSTRAINT prices_pkey PRIMARY KEY (company_id, ts);


--
-- Name: prices_ticks prices_ticks_pkey; Type: CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.prices_ticks
    ADD CONSTRAINT prices_ticks_pkey PRIMARY KEY (company_id, sim_day, ts);


--
-- Name: candles_1h_day_idx; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX candles_1h_day_idx ON market.candles_1h USING btree (sim_day);


--
-- Name: idx_companies_industry_status; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX idx_companies_industry_status ON market.companies USING btree (industry_id, status);


--
-- Name: idx_companies_status; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX idx_companies_status ON market.companies USING btree (status);


--
-- Name: idx_events_company_ts; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX idx_events_company_ts ON market.events USING btree (company_id, ts DESC);


--
-- Name: idx_prices_company_ts_desc; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX idx_prices_company_ts_desc ON market.prices USING btree (company_id, ts DESC);


--
-- Name: idx_prices_ts_desc; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX idx_prices_ts_desc ON market.prices USING btree (ts DESC);


--
-- Name: ix_market_companies_industry; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX ix_market_companies_industry ON market.companies USING btree (industry_id);


--
-- Name: ix_market_companies_status_listed; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX ix_market_companies_status_listed ON market.companies USING btree (status, listed);


--
-- Name: ix_market_events_company_ts; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX ix_market_events_company_ts ON market.events USING btree (company_id, ts DESC);


--
-- Name: ix_market_prices_ts; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX ix_market_prices_ts ON market.prices USING btree (ts DESC);


--
-- Name: prices_ticks_company_ts_idx; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX prices_ticks_company_ts_idx ON market.prices_ticks USING btree (company_id, ts DESC);


--
-- Name: prices_ticks_day_idx; Type: INDEX; Schema: market; Owner: -
--

CREATE INDEX prices_ticks_day_idx ON market.prices_ticks USING btree (sim_day);


--
-- Name: ux_market_companies_name; Type: INDEX; Schema: market; Owner: -
--

CREATE UNIQUE INDEX ux_market_companies_name ON market.companies USING btree (name);


--
-- Name: candles_1h candles_1h_company_id_fkey; Type: FK CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.candles_1h
    ADD CONSTRAINT candles_1h_company_id_fkey FOREIGN KEY (company_id) REFERENCES market.companies(company_id);


--
-- Name: companies companies_industry_id_fkey; Type: FK CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.companies
    ADD CONSTRAINT companies_industry_id_fkey FOREIGN KEY (industry_id) REFERENCES market.industries(industry_id);


--
-- Name: events events_company_id_fkey; Type: FK CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.events
    ADD CONSTRAINT events_company_id_fkey FOREIGN KEY (company_id) REFERENCES market.companies(company_id);


--
-- Name: industry_seasonality industry_seasonality_industry_id_fkey; Type: FK CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.industry_seasonality
    ADD CONSTRAINT industry_seasonality_industry_id_fkey FOREIGN KEY (industry_id) REFERENCES market.industries(industry_id);


--
-- Name: prices prices_company_id_fkey; Type: FK CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.prices
    ADD CONSTRAINT prices_company_id_fkey FOREIGN KEY (company_id) REFERENCES market.companies(company_id);


--
-- Name: prices_ticks prices_ticks_company_id_fkey; Type: FK CONSTRAINT; Schema: market; Owner: -
--

ALTER TABLE ONLY market.prices_ticks
    ADD CONSTRAINT prices_ticks_company_id_fkey FOREIGN KEY (company_id) REFERENCES market.companies(company_id);


--
-- PostgreSQL database dump complete
--

\unrestrict 9a24LCvsn9VBek5d5kkilbXWa5ZzKOOTzBTfVPH8lPO5kx1PgD6T3eRVv2uPH3Z

