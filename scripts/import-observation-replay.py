"""Import preserved two-seat observations/events; never reconstruct missing truth.

Usage: python scripts/import-observation-replay.py --evidence PATH --output PATH
       [--deploy web/replays] [--provenance source-version.json]
Only Python's standard library is used. Originals remain read-only.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path


def rows(path):
    return [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines() if line.strip()]


def build(evidence, provenance=None):
    observations = rows(evidence / 'observations.jsonl')
    if not observations or observations[0].get('scenario_id') != 'dual_carrier_hotseat':
        raise ValueError('This lesson importer is for the archived Twin Shields hotseat battle only')
    raw_events = rows(evidence / 'events.jsonl')
    frames = {s: {} for s in ('Blue', 'Red')}
    for row in observations:
        side = row['view']
        if side not in frames or any(u['side'] != side for u in row['units']):
            raise ValueError('Invalid side projection')
        frames[side][row['tick']] = {k: row[k] for k in ('tick', 'units', 'contacts', 'mission', 'over', 'result')}
    events = {}
    for row in raw_events:
        event = {k: v for k, v in row.items() if k not in ('wall_s', 'view')}
        if event['seq'] in events and events[event['seq']] != event:
            raise ValueError('Conflicting event sequence')
        events[event['seq']] = event
    if sorted(events) != list(range(1, max(events) + 1)):
        raise ValueError('Missing event sequences; cannot label this recording complete')
    if any(not frames[s] or not frames[s][max(frames[s])]['over'] for s in frames):
        raise ValueError('This importer requires completed two-sided recordings')
    counts = {s: len(frames[s]) for s in frames}
    gap = max(b-a for s in frames for a,b in zip(sorted(frames[s]), sorted(frames[s])[1:]))
    hashes = {name: hashlib.sha256((evidence/name).read_bytes()).hexdigest()
              for name in ('observations.jsonl', 'events.jsonl', 'receipts.jsonl')}
    archive = {
        'format': 'harpoon-observation-replay', 'version': 1,
        'id': 'twin-shields-20261019',
        'title': {'zh': '双盾行动 · 六小时双边手操', 'en': 'Twin Shields · Six-hour hotseat'},
        'scenario_id': observations[0]['scenario_id'],
        'theater': observations[0]['theater'],
        'source': {'mode': 'recorded_observations', 'seed': 20261019,
                   'original_observations': len(observations), 'frames_by_side': counts,
                   'max_sample_gap_s': gap, 'unique_events': len(events),
                   'event_sequences_complete': True, 'input_sha256': hashes,
                   'versions': json.loads(provenance.read_text(encoding='utf-8')) if provenance else None},
        'frames': {s: [frames[s][tick] for tick in sorted(frames[s])] for s in frames},
        'events': [events[n] for n in sorted(events)],
        'bookmarks': [
            {'tick': 0, 'label': {'zh': '开局与迷雾', 'en': 'Start and fog of war'}, 'views': ['Blue','Red','God']},
            {'tick': 1078, 'label': {'zh': '辽宁号受击', 'en': 'Liaoning hit'}, 'views': ['Blue','God']},
            {'tick': 1079, 'label': {'zh': '山东号受击', 'en': 'Shandong hit'}, 'views': ['Blue','God']},
            {'tick': 14127, 'label': {'zh': '福特号突入前', 'en': 'Before Ford advances'}, 'views': ['Red','God']},
            {'tick': 20380, 'label': {'zh': '福特号开始占区', 'en': 'Ford begins holding'}, 'views': ['Red','God']},
            {'tick': 20560, 'label': {'zh': '真实任务终局', 'en': 'Recorded mission outcome'}, 'views': ['Blue','Red','God']},
        ]}
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--deploy', type=Path)
    parser.add_argument('--provenance', type=Path)
    args = parser.parse_args()
    archive = build(args.evidence, args.provenance)
    payload = json.dumps(archive, ensure_ascii=False, separators=(',', ':')).encode('utf-8')
    packed = gzip.compress(payload, compresslevel=9, mtime=0)
    args.output.mkdir(parents=True, exist_ok=True)
    name = archive['id'] + '.replay.json.gz'
    (args.output/name).write_bytes(packed)
    digest = hashlib.sha256(packed).hexdigest()
    catalog = {'replays': [{'id': archive['id'], 'title': archive['title'], 'file': name,
                            'bytes': len(packed), 'sha256': digest}]}
    (args.output/'catalog.json').write_text(json.dumps(catalog, ensure_ascii=False, indent=2), encoding='utf-8')
    (args.output/'import-validation.json').write_text(json.dumps({
        **archive['source'], 'compressed_bytes': len(packed), 'json_bytes': len(payload),
        'archive_sha256': digest, 'last_tick': max(f['tick'] for f in archive['frames']['Blue']),
        'passed': json.loads(gzip.decompress(packed)) == archive
    }, ensure_ascii=False, indent=2), encoding='utf-8')
    if args.deploy:
        args.deploy.mkdir(parents=True, exist_ok=True)
        for filename in (name, 'catalog.json'):
            (args.deploy/filename).write_bytes((args.output/filename).read_bytes())
    print(json.dumps({'file': str(args.output/name), 'sha256': digest, 'bytes': len(packed),
                      'frames': archive['source']['frames_by_side'], 'events': len(archive['events'])}))


if __name__ == '__main__':
    main()
