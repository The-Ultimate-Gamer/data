#!/usr/bin/env python3
"""Generate the public JSON documents from the SQLite database.

Unchanged documents keep their previous generatedAt; documents for removed
players or tournaments are deleted.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from ugdata import DEFAULT_DB, DEFAULT_OUT, connect, export


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", type=Path, default=DEFAULT_DB)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = parser.parse_args(argv)

    if not args.db.exists():
        raise SystemExit(f"{args.db} does not exist")
    conn = connect(args.db)
    report = export(conn, args.out)
    conn.close()
    print(
        f"Exported to {args.out}: {len(report['written'])} written, "
        f"{len(report['unchanged'])} unchanged, {len(report['removed'])} removed"
    )
    for rel in report["written"]:
        print(f"  + {rel}")
    for rel in report["removed"]:
        print(f"  - {rel}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
