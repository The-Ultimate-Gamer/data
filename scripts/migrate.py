#!/usr/bin/env python3
"""Bring an existing database up to date with schema/schema.sql.

Adds tables and indexes that don't exist yet (the schema uses IF NOT EXISTS).
Column changes to existing tables need an explicit ALTER TABLE step below.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from ugdata import DEFAULT_DB, connect, create_schema


def migrate(path: Path) -> None:
    conn = connect(path)
    with conn:
        create_schema(conn)
    conn.close()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", type=Path, default=DEFAULT_DB)
    args = parser.parse_args(argv)
    if not args.db.exists():
        raise SystemExit(f"{args.db} does not exist")
    migrate(args.db)
    print(f"Migrated {args.db}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
