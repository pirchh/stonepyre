#!/usr/bin/env bash
# Creates and migrates the stonepyre_market database.
# Usage: ./setup_db.sh [db_name]   (defaults to stonepyre_market)
set -euo pipefail

DB="${1:-stonepyre_market}"
SQL_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "==> Creating database '$DB' (skips if exists)..."
createdb "$DB" 2>/dev/null && echo "    created" || echo "    already exists, continuing"

run() {
    echo "  -> $1"
    psql -d "$DB" -f "$SQL_DIR/$1"
}

echo ""
echo "==> auth schema"
run auth/001_schema.sql
run auth/002_tables.sql
run auth/003_indexes.sql
run auth/005_core.sql
run auth/20260513_02_admin_flag.sql

echo ""
echo "==> game schema"
run game/001_schema.sql
run game/002_tables.sql
run game/003_indexes.sql
run game/004_functions.sql
run game/20260513_01_bag_slots.sql
run game/20260515_01_bank.sql

echo ""
echo "==> market schema"
run market/001_schema.sql
run market/005_core.sql          # replaces 002_tables — has trend cols + weight
run market/003_indexes.sql
run market/004_views.sql
run market/006_clock.sql
run market/007_industry_seasonality.sql
run market/008_company_lifecycle_helpers.sql
run market/009_tick_candles.sql
run market/010_get_clock.sql

echo ""
echo "==> portfolio schema"
run portfolio/001_schema.sql
run portfolio/002_tables.sql
run portfolio/003_indexes.sql
run portfolio/005_core.sql
run portfolio/006_orders.sql

echo ""
echo "==> Done. Database '$DB' is ready."
