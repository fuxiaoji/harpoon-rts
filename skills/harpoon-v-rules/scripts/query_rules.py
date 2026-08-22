from __future__ import annotations

import argparse
import re
import sqlite3
from pathlib import Path


def find_database() -> Path:
    for root in (Path.cwd(), *Path.cwd().parents):
        candidate = root / "resources" / "rag" / "harpoon_rules.sqlite3"
        if candidate.exists():
            return candidate
    fallback = Path(r"D:\desktop\鱼叉\resources\rag\harpoon_rules.sqlite3")
    if fallback.exists():
        return fallback
    raise FileNotFoundError("harpoon_rules.sqlite3 not found; run scripts/build_rule_rag.py")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("query")
    parser.add_argument("--limit", type=int, default=8)
    parser.add_argument("--core-only", action="store_true")
    args = parser.parse_args()
    database = find_database()
    terms = [term for term in re.split(r"\s+", args.query.strip()) if term]
    if not terms:
        raise SystemExit("query must not be empty")
    if any(len(term) < 3 for term in terms):
        clauses = ["c.text LIKE ?" for _ in terms]
        sql = """
          SELECT c.chunk_id, d.title, d.path, c.section, substr(c.text, 1, 420)
          FROM chunks c JOIN documents d USING(document_id)
          WHERE """ + " AND ".join(clauses)
        parameters = [f"%{term}%" for term in terms]
        if args.core_only:
            sql += " AND c.core = 1"
        sql += " ORDER BY c.document_id, c.page LIMIT ?"
    else:
        match = " AND ".join(f'"{term.replace(chr(34), chr(34) + chr(34))}"' for term in terms)
        sql = """
          SELECT chunk_id, title, path, section, snippet(chunks_fts, 4, '[', ']', '…', 24)
          FROM chunks_fts WHERE chunks_fts MATCH ?
        """
        parameters = [match]
        if args.core_only:
            sql += " AND core = 1"
        sql += " ORDER BY rank LIMIT ?"
    parameters.append(args.limit)
    with sqlite3.connect(database) as connection:
        rows = connection.execute(sql, parameters).fetchall()
    for chunk_id, title, path, section, snippet in rows:
        print(f"{chunk_id} | {title} | {path} | {section}\n{snippet}\n")


if __name__ == "__main__":
    main()
