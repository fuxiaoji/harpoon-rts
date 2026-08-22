# Four-way rule audit

For every base-rule table, maintain one audit row with four independent checks:

1. **Source** - core document, printed section/page, PDF page, version, and visual verification.
2. **Data** - machine-readable table/schema with every cell, modifier, priority, and rounding instruction.
3. **Execution** - engine call site that consumes the structured data without duplicating constants.
4. **Tests** - boundary cells, modifier composition, illegal inputs, deterministic replay, and UI trace.

Use these statuses:

- `indexed`: retrievable text exists, layout not verified.
- `verified_source`: table and notes visually verified.
- `structured`: complete machine-readable transcription exists.
- `executable`: engine consumes it and produces an auditable event.
- `tested`: source-derived tests pass.
- `blocked`: a named ambiguity prevents safe execution.

“All rules entered” is true only when every mandatory base-rule audit row is at least `tested`, or explicitly `not_applicable` with a source-backed reason. Optional rules must be listed separately and cannot silently disappear.
