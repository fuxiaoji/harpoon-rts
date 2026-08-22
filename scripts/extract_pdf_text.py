"""Extract page-delimited UTF-8 text for research, never as layout truth."""

from __future__ import annotations

import argparse
from pathlib import Path

from pypdf import PdfReader


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("pdf", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    reader = PdfReader(str(args.pdf))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8", newline="\n") as stream:
        for index, page in enumerate(reader.pages, start=1):
            stream.write(f"\n\n===== PDF PAGE {index} =====\n\n")
            stream.write(page.extract_text() or "")
    print(f"Extracted {len(reader.pages)} pages to {args.output}")


if __name__ == "__main__":
    main()
