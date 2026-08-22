# Corpus contract

The generated corpus lives under `resources/rag/`:

- `manifest.json`: source identity, SHA-256, page counts, and corpus counts.
- `core_chunks.jsonl`: page-aware overlapping chunks for the three mandatory manuals.
- `table_candidates.jsonl`: automatically detected table/figure headings; candidates are not verified until visual review.
- `harpoon_rules.sqlite3`: all PDF content in SQLite FTS5 trigram search, plus ZIP entry names and document metadata.

Rebuild with the bundled Python runtime or an environment containing `pdfplumber`:

```powershell
python scripts/build_rule_rag.py
```

Core chunks preserve page boundaries and use layout-aware extraction. Other PDFs use the same searchable page index but are reference-only unless promoted through source review.
