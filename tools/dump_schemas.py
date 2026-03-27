#!/usr/bin/env python3
"""
Dump each Postgres schema into its own schema-only .sql file.

Outputs:
  <output_dir>/<schema>.sql

Includes:
  tables, columns, types, sequences, views, functions, triggers, indexes, constraints, etc.
(via pg_dump --schema-only)

Usage (from Stonepyre/tools):
  python dump_schemas.py
"""

from __future__ import annotations

import os
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional


# ------------------------------------------------------------
# CONFIG (hardcoded as requested)
# ------------------------------------------------------------

@dataclass(frozen=True)
class DbConfig:
    host: str = "127.0.0.1"
    port: int = 5432
    dbname: str = "stonepyre_market"
    user: str = "postgres"
    password: str = "postgres"


TOOLS_DIR = Path(r"C:\Users\ryanj\Development\GameProjects\Stonepyre\tools")
OUTPUT_DIR = Path(r"C:\Users\ryanj\Development\GameProjects\Stonepyre\libs\sql\python_outputs")

# If pg_dump is not on PATH, set this to your pg_dump.exe full path, e.g.:
# PG_DUMP_EXE = r"C:\Program Files\PostgreSQL\16\bin\pg_dump.exe"
PG_DUMP_EXE: Optional[str] = r"C:\Program Files\PostgreSQL\17\bin\pg_dump.exe"

# Schemas to exclude (system + common internal)
EXCLUDE_SCHEMAS = {
    "pg_catalog",
    "information_schema",
    "pg_toast",
    "pg_temp_1",
    "pg_toast_temp_1",
}


# ------------------------------------------------------------
# Helpers
# ------------------------------------------------------------

def _find_pg_dump() -> str:
    if PG_DUMP_EXE:
        p = Path(PG_DUMP_EXE)
        if p.exists():
            return str(p)
        raise FileNotFoundError(f"PG_DUMP_EXE was set but not found: {p}")

    which = shutil.which("pg_dump")
    if which:
        return which

    # Common Windows install locations (best-effort)
    candidates = [
        r"C:\Program Files\PostgreSQL\16\bin\pg_dump.exe",
        r"C:\Program Files\PostgreSQL\15\bin\pg_dump.exe",
        r"C:\Program Files\PostgreSQL\14\bin\pg_dump.exe",
        r"C:\Program Files\PostgreSQL\13\bin\pg_dump.exe",
    ]
    for c in candidates:
        if Path(c).exists():
            return c

    raise FileNotFoundError(
        "pg_dump not found. Add it to PATH, or set PG_DUMP_EXE to full path."
    )


def list_user_schemas(cfg: DbConfig) -> List[str]:
    """
    List non-system schemas from the DB.
    Uses psycopg (v3) if available; falls back to psycopg2.
    """
    query = """
    SELECT nspname
    FROM pg_namespace
    WHERE nspname NOT LIKE 'pg_%'
      AND nspname <> 'information_schema'
    ORDER BY nspname
    """

    # Try psycopg (v3)
    try:
        import psycopg  # type: ignore
        conn_str = f"host={cfg.host} port={cfg.port} dbname={cfg.dbname} user={cfg.user} password={cfg.password}"
        with psycopg.connect(conn_str) as conn:
            with conn.cursor() as cur:
                cur.execute(query)
                rows = cur.fetchall()
        schemas = [r[0] for r in rows if r[0] not in EXCLUDE_SCHEMAS]
        return schemas
    except ImportError:
        pass

    # Fallback psycopg2
    try:
        import psycopg2  # type: ignore
        conn = psycopg2.connect(
            host=cfg.host,
            port=cfg.port,
            dbname=cfg.dbname,
            user=cfg.user,
            password=cfg.password,
        )
        try:
            with conn.cursor() as cur:
                cur.execute(query)
                rows = cur.fetchall()
            schemas = [r[0] for r in rows if r[0] not in EXCLUDE_SCHEMAS]
            return schemas
        finally:
            conn.close()
    except ImportError as e:
        raise RuntimeError(
            "Neither psycopg (v3) nor psycopg2 is installed. "
            "Install one: pip install psycopg[binary]  OR  pip install psycopg2-binary"
        ) from e


def dump_schema(cfg: DbConfig, schema: str, out_file: Path, pg_dump_path: str) -> None:
    """
    Runs pg_dump schema-only for one schema into out_file.
    """
    out_file.parent.mkdir(parents=True, exist_ok=True)

    env = os.environ.copy()
    env["PGPASSWORD"] = cfg.password  # avoids interactive prompt

    cmd = [
        pg_dump_path,
        "-h", cfg.host,
        "-p", str(cfg.port),
        "-U", cfg.user,
        "-d", cfg.dbname,
        "--schema-only",
        "--no-owner",
        "--no-privileges",
        "--schema", schema,
    ]

    # Write directly to file to avoid encoding/size issues
    with out_file.open("wb") as f:
        proc = subprocess.run(cmd, env=env, stdout=f, stderr=subprocess.PIPE)

    if proc.returncode != 0:
        err = proc.stderr.decode("utf-8", errors="replace")
        raise RuntimeError(f"pg_dump failed for schema '{schema}':\n{err}")


def main() -> int:
    cfg = DbConfig()

    print("=== Stonepyre Postgres Schema Dumper ===")
    print(f"DB:   postgres://{cfg.user}:***@{cfg.host}:{cfg.port}/{cfg.dbname}")
    print(f"OUT:  {OUTPUT_DIR}")

    pg_dump_path = _find_pg_dump()
    print(f"pg_dump: {pg_dump_path}")

    schemas = list_user_schemas(cfg)
    if not schemas:
        print("No user schemas found.")
        return 0

    print(f"Schemas ({len(schemas)}): {', '.join(schemas)}")

    # Dump each schema to <schema>.sql
    for schema in schemas:
        out_file = OUTPUT_DIR / f"{schema}.sql"
        print(f"Dumping schema '{schema}' -> {out_file.name}")
        dump_schema(cfg, schema, out_file, pg_dump_path)

    print("Done.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())