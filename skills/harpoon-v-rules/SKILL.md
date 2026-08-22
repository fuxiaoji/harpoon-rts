---
name: harpoon-v-rules
description: Retrieve, verify, and implement Harpoon V rules using this project's three Chinese core manuals, source-page citations, table coverage, and engine audit trail. Use for Harpoon rules questions, adjudication tables, scenario legality, engine changes, tutorials, and AI action explanations; do not use for unrelated naval history or generic game design.
---

# Harpoon V Rule Verification

Treat the three core manuals as the mandatory retrieval set:

1. `1-3.（核心）兵棋推演手册26版.pdf` - full rules and context.
2. `2-2.（推演手册2026版本）Harpoon_V_Player_s_Handbook_(Chinese_Language).pdf` - consolidated player tables and quick procedures.
3. `3-2.（扩展损伤手册26版）Harpoon_V_Expanded_Critical_Hit_Tables_(Chinese_Language).pdf` - expanded critical-hit resolution.

The repository rule in `agent.md` still controls authority: when a Chinese translation conflicts with the 2026-01-01 English digital rules, use the English rule and record the discrepancy.

## Required workflow

- Before answering or changing adjudication logic, query `resources/rag/harpoon_rules.sqlite3` with `scripts/query_rules.py`. Search both the rule concept and known table name.
- Read the returned source page in the original PDF. Text extraction is retrieval evidence, not proof of table layout; visually render every table page before marking a table verified.
- Assign or reuse a stable `rule_id`. Record document, PDF page, printed page/section, version, conditions, modifiers, priority, rounding, and verification state.
- Keep rule data outside UI/controller code. An engine rule is complete only when structured data, engine call site, event trace, and boundary tests all exist.
- If any required cell, modifier, priority, or rounding rule is unresolved, block automatic adjudication and add an entry to `docs/rules/open_questions.md`.
- Never claim the manuals are fully implemented based only on indexing. Use the four-way audit described in [references/audit.md](references/audit.md).

## Retrieval

Run from the repository root:

```powershell
python skills/harpoon-v-rules/scripts/query_rules.py "反舰导弹攻击表"
python skills/harpoon-v-rules/scripts/query_rules.py "火灾 进水 损管" --core-only
```

Read [references/corpus.md](references/corpus.md) when rebuilding or diagnosing the RAG corpus. Read [references/audit.md](references/audit.md) whenever assessing completeness or changing engine coverage.
