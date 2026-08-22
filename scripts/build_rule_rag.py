"""Build the Harpoon V core-rule RAG corpus and a searchable all-source index."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sqlite3
import zipfile
from collections.abc import Iterator
from pathlib import Path

import pdfplumber
from pypdf import PdfReader

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "【20260405】鱼叉5（本体+中美俄数据库）"
OUTPUT = ROOT / "resources" / "rag"
CORE_FILES = {
    "core_rules_cn": "1-3.（核心）兵棋推演手册26版.pdf",
    "player_handbook_cn": "2-2.（推演手册2026版本）Harpoon_V_Player_s_Handbook_(Chinese_Language).pdf",
    "expanded_critical_hits_cn": "3-2.（扩展损伤手册26版）Harpoon_V_Expanded_Critical_Hit_Tables_(Chinese_Language).pdf",
}
TABLE_PATTERN = re.compile(r"^[^。；]{0,72}(?:表|图)(?:\s|$|（|\()")
SECTION_PATTERN = re.compile(r"^(?:第[一二三四五六七八九十]+章|\d+(?:\.\d+){0,3}\s+|Chapter\s+)")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def chunk_text(text: str, size: int = 1100, overlap: int = 180) -> Iterator[str]:
    clean = re.sub(r"[ \t]+", " ", text).strip()
    if not clean:
        return
    start = 0
    while start < len(clean):
        end = min(len(clean), start + size)
        if end < len(clean):
            split = max(clean.rfind("\n", start + size // 2, end), clean.rfind("。", start, end))
            if split > start:
                end = split + 1
        yield clean[start:end].strip()
        if end >= len(clean):
            break
        start = max(start + 1, end - overlap)


def page_heading(text: str) -> str:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    for line in lines[:18]:
        if SECTION_PATTERN.match(line) or (len(line) <= 48 and "表" in line):
            return line
    return lines[0][:80] if lines else ""


def table_candidates(text: str) -> list[str]:
    result: list[str] = []
    for raw in text.splitlines():
        line = re.sub(r"\s+", " ", raw).strip(" •·")
        if 2 <= len(line) <= 80 and TABLE_PATTERN.search(line) and line not in result:
            result.append(line)
    return result


def create_database(path: Path) -> sqlite3.Connection:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        path.unlink()
    connection = sqlite3.connect(path)
    connection.executescript(
        """
        CREATE TABLE documents (
          document_id TEXT PRIMARY KEY, title TEXT NOT NULL, path TEXT NOT NULL,
          sha256 TEXT NOT NULL, pages INTEGER, core INTEGER NOT NULL, role TEXT NOT NULL
        );
        CREATE TABLE chunks (
          chunk_id TEXT PRIMARY KEY, document_id TEXT NOT NULL, page INTEGER,
          section TEXT, text TEXT NOT NULL, core INTEGER NOT NULL,
          FOREIGN KEY(document_id) REFERENCES documents(document_id)
        );
        CREATE VIRTUAL TABLE chunks_fts USING fts5(
          chunk_id UNINDEXED, title, path, section, text, core UNINDEXED, tokenize='trigram'
        );
        CREATE TABLE archive_entries (archive_path TEXT, entry_name TEXT, size INTEGER);
        """
    )
    return connection


def index_pdf(
    connection: sqlite3.Connection,
    path: Path,
    document_id: str,
    role: str,
    core: bool,
    core_chunks: list[dict[str, object]],
    tables: list[dict[str, object]],
) -> int:
    relative = path.relative_to(ROOT).as_posix()
    document_sha256 = sha256(path)
    if core:
        pdf = pdfplumber.open(path)
        pages = pdf.pages

        def extract(page: object) -> str:
            return page.extract_text(x_tolerance=2, y_tolerance=2, layout=True) or ""  # type: ignore[attr-defined]
    else:
        pdf = PdfReader(path)
        pages = pdf.pages

        def extract(page: object) -> str:
            return page.extract_text() or ""  # type: ignore[attr-defined]
    try:
        page_count = len(pages)
        connection.execute(
            "INSERT INTO documents VALUES (?,?,?,?,?,?,?)",
            (document_id, path.stem, relative, document_sha256, page_count, int(core), role),
        )
        for page_number, page in enumerate(pages, 1):
            text = extract(page)
            heading = page_heading(text)
            for sequence, chunk in enumerate(chunk_text(text), 1):
                chunk_id = f"{document_id}:p{page_number:03d}:c{sequence:02d}"
                row = {
                    "chunk_id": chunk_id,
                    "document_id": document_id,
                    "title": path.stem,
                    "source_path": relative,
                    "page": page_number,
                    "section": heading,
                    "role": role,
                    "sha256": document_sha256,
                    "text": chunk,
                }
                connection.execute(
                    "INSERT INTO chunks VALUES (?,?,?,?,?,?)",
                    (chunk_id, document_id, page_number, heading, chunk, int(core)),
                )
                connection.execute(
                    "INSERT INTO chunks_fts VALUES (?,?,?,?,?,?)",
                    (chunk_id, path.stem, relative, heading, chunk, int(core)),
                )
                if core:
                    core_chunks.append(row)
            if core:
                for title in table_candidates(text):
                    tables.append(
                        {
                            "table_id": f"{document_id}:p{page_number:03d}:{len(tables) + 1:04d}",
                            "document_id": document_id,
                            "page": page_number,
                            "title": title,
                            "section": heading,
                            "source_path": relative,
                            "verification": "candidate_requires_visual_check",
                        }
                    )
    finally:
        if core:
            pdf.close()
    return page_count


def build(source: Path = SOURCE, output: Path = OUTPUT) -> dict[str, object]:
    output.mkdir(parents=True, exist_ok=True)
    connection = create_database(output / "harpoon_rules.sqlite3")
    core_chunks: list[dict[str, object]] = []
    tables: list[dict[str, object]] = []
    manifest: dict[str, object] = {"schema_version": "1.0.0", "core": [], "other": []}
    core_by_name = {name: (document_id, role) for role, name in CORE_FILES.items() for document_id in [role]}
    for path in sorted(source.rglob("*")):
        if not path.is_file():
            continue
        if path.suffix.lower() == ".pdf":
            if path.name in core_by_name:
                document_id, role = core_by_name[path.name]
                is_core = True
            else:
                document_id = f"source_{hashlib.sha1(path.as_posix().encode()).hexdigest()[:12]}"
                role, is_core = "searchable_reference", False
            try:
                pages = index_pdf(connection, path, document_id, role, is_core, core_chunks, tables)
                connection.commit()
            except Exception as exc:
                if is_core:
                    raise
                connection.rollback()
                manifest["other"].append(
                    {
                        "path": path.relative_to(ROOT).as_posix(),
                        "sha256": sha256(path),
                        "index_error": f"{type(exc).__name__}: {exc}",
                    }
                )
                continue
            target = "core" if is_core else "other"
            manifest[target].append(
                {"document_id": document_id, "path": path.relative_to(ROOT).as_posix(), "pages": pages, "sha256": sha256(path)}
            )
        elif path.suffix.lower() == ".zip":
            with zipfile.ZipFile(path) as archive:
                for item in archive.infolist():
                    connection.execute(
                        "INSERT INTO archive_entries VALUES (?,?,?)",
                        (path.relative_to(ROOT).as_posix(), item.filename, item.file_size),
                    )
        else:
            manifest["other"].append(
                {"path": path.relative_to(ROOT).as_posix(), "bytes": path.stat().st_size, "sha256": sha256(path)}
            )
    connection.commit()
    counts = {
        "documents": connection.execute("SELECT count(*) FROM documents").fetchone()[0],
        "chunks": connection.execute("SELECT count(*) FROM chunks").fetchone()[0],
        "core_chunks": len(core_chunks),
        "table_candidates": len(tables),
        "archive_entries": connection.execute("SELECT count(*) FROM archive_entries").fetchone()[0],
    }
    connection.close()
    manifest["counts"] = counts
    (output / "core_chunks.jsonl").write_text(
        "".join(json.dumps(item, ensure_ascii=False) + "\n" for item in core_chunks), encoding="utf-8"
    )
    (output / "table_candidates.jsonl").write_text(
        "".join(json.dumps(item, ensure_ascii=False) + "\n" for item in tables), encoding="utf-8"
    )
    (output / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    return counts


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=SOURCE)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    print(json.dumps(build(args.source, args.output), ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
