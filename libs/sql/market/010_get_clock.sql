CREATE OR REPLACE FUNCTION market.get_clock()
RETURNS TABLE(sim_day integer, minute_of_day integer, is_open boolean)
LANGUAGE sql AS $$
  SELECT sim_day, minute_of_day, is_open
  FROM market.clock
  WHERE id = 1
$$;
