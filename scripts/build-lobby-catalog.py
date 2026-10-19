"""Build public lobby metadata from authored scenario files (no combat changes)."""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
entries = {}
for path in sorted((ROOT/'scenarios').glob('*.json')):
    raw = path.read_bytes()
    scenario = json.loads(raw)
    units = scenario.get('units', [])
    entries[scenario['id']] = {
        'file': path.name, 'source_sha256': hashlib.sha256(raw).hexdigest(),
        'duration_s': scenario.get('duration_s'), 'seed': scenario.get('seed'),
        'initial_units': len(units),
        'sides': {s: sum(u['side'] == s for u in units) for s in ('Blue', 'Red')},
        'automated_side': scenario.get('context', {}).get('automated_side'),
        'satellite': bool(scenario.get('context', {}).get('theater')),
    }
(ROOT/'web/lobby-catalog.json').write_text(json.dumps({'version': 1, 'scenarios': entries}, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print(f'Lobby metadata: {len(entries)} scenarios')
