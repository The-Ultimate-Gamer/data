#!/usr/bin/env python3
"""Create an empty database from schema/schema.sql."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from ugdata import DEFAULT_DB, SCHEMA_VERSION, connect, create_schema


def init_db(path: Path, force: bool = False) -> None:
    if path.exists():
        if not force:
            raise SystemExit(f"{path} already exists (use --force to recreate it)")
        path.unlink()
    path.parent.mkdir(parents=True, exist_ok=True)
    with connect(path) as conn:
        create_schema(conn)
        conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?)", (str(SCHEMA_VERSION),)
        )
    conn.close()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", type=Path, default=DEFAULT_DB, help=f"database path (default: {DEFAULT_DB})")
    parser.add_argument("--force", action="store_true", help="delete and recreate an existing database")
    args = parser.parse_args(argv)
    init_db(args.db, args.force)
    print(f"Created {args.db}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
