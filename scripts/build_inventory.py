"""Build a reproducible catalog without modifying source material."""

from __future__ import annotations

import csv
import hashlib
from pathlib import Path

from pypdf import PdfReader


ROOT = Path(__file__).resolve().parents[1]
SOURCE_DIR = ROOT / "【20260405】鱼叉5（本体+中美俄数据库）"
OUTPUT = ROOT / "resources" / "catalog" / "inventory.csv"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(4 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def classify(relative: str) -> str:
    name = relative.lower()
    if "鱼叉5/" in relative.replace("\\", "/") or any(token in name for token in ("海军运筹", "作战运筹", "舰队战术")):
        return "research"
    if any(token in name for token in ("errata", "change_log", "订正")):
        return "rules/errata"
    if any(token in name for token in ("critical_hit", "致命损伤", "damage_point")):
        return "rules/damage"
    if any(token in name for token in ("player_s_handbook", "推演手册速查", "worksheet", "calculator", "blank_forms", "h5_form")):
        return "rules/player-aid"
    if any(token in name for token in ("hpnv_rules", "兵棋推演手册21版", "兵棋推演手册26版", "harpoon_v_oct")):
        return "rules/core"
    if any(token in name for token in ("aircraft", "航空")):
        return "data/aircraft"
    if any(token in name for token in ("navy", "navies", "maritime_forces", "海军", "自卫队", "舰艇数据库", "台军数据")):
        return "data/navy"
    if any(token in name for token in ("tattletail", "high_tide", "dance_of_the_vampires", "south_atlantic_war", "scenario")):
        return "scenario"
    if any(token in name for token in ("harpoonv.exe", "save.dat", "east_sea.png")):
        return "software"
    if "官方辅助" in relative:
        return "research"
    return "archive"


def pdf_pages(path: Path) -> str:
    if path.suffix.lower() != ".pdf":
        return ""
    try:
        return str(len(PdfReader(str(path)).pages))
    except Exception as exc:  # Preserve the error in the catalog for review.
        return f"ERROR:{type(exc).__name__}"


def main() -> None:
    paths = [p for p in SOURCE_DIR.rglob("*") if p.is_file()]
    paths.extend(
        p
        for p in ROOT.iterdir()
        if p.is_file() and p.suffix.lower() in {".pdf", ".zip"}
    )
    paths = sorted(paths, key=lambda p: str(p).casefold())
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    with OUTPUT.open("w", encoding="utf-8-sig", newline="") as stream:
        writer = csv.writer(stream)
        writer.writerow(["relative_path", "category", "extension", "bytes", "sha256", "pdf_pages"])
        for path in paths:
            relative = path.relative_to(ROOT).as_posix()
            writer.writerow([relative, classify(relative), path.suffix.lower(), path.stat().st_size, sha256(path), pdf_pages(path)])
    print(f"Wrote {len(paths)} rows to {OUTPUT}")


if __name__ == "__main__":
    main()
