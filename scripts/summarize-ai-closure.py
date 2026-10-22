"""Post-match referee metrics. Never used as a live player observation."""
import argparse
import collections
import json
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('output', type=Path)
p.add_argument('matches', nargs='+', type=Path)
a = p.parse_args()
rows = []
for path in a.matches:
    j = json.loads(path.read_text(encoding='utf-8-sig'))
    audit = j['flight_audit']
    assert all(e['seq'] == i + 1 for i, e in enumerate(j['events'])), path
    sides = {}
    for side in ('Blue', 'Red'):
        flights = [f for f in audit['flights'].values() if f['side'] == side]
        strike = [f for f in flights if f['mission'] == 'strike']
        blocks = collections.Counter()
        for f in strike:
            blocks.update(f['blocked_seconds'])
        tracked = sum(f['tracked_seconds'] for f in strike)
        fresh = sum(f['fresh_track_seconds'] for f in strike)
        bins = collections.defaultdict(list)
        for r in j['terminal_entries']:
            if r['side'] == side:
                bins[(r['target'], r['tick'] // 60)].append(r)
        mixed = [v for v in bins.values() if len({r['shooter_kind'] for r in v}) > 1]
        sides[side] = dict(
            strike_sorties=len(strike), actual_air_asm_rounds=sum(f['actual_asm_rounds'] for f in flights),
            strike_sorties_with_release=sum(f['actual_asm_rounds'] > 0 for f in strike),
            strike_fates=dict(collections.Counter(f['fate'] for f in strike)),
            support_fates=dict(collections.Counter(f['fate'] for f in flights if f['mission'] in ('recon', 'asw'))),
            strike_tracked_seconds=tracked, strike_fresh_track_seconds=fresh,
            fresh_track_fraction=fresh / tracked if tracked else None,
            longest_fresh_track_seconds=max((f['longest_fresh_run'] for f in strike), default=0),
            release_envelope_seconds=sum(f['release_envelope_seconds'] for f in strike),
            blocker_seconds=dict(blocks), mixed_source_target_minute_bins=len(mixed),
            max_mixed_source_terminal_entries=max(map(len, mixed), default=0),
            audit_actions=dict(collections.Counter(e['rule'] for e in j['events']
                if e.get('side') == side and e.get('domain') == 'tactical_ai')))
    rows.append(dict(file=path.name, seed=j['seed'], tick=j['tick'], result=j['result'],
        world_fnv64=j['world_fnv64'], events=len(j['events']), sea_position_checks=j['sea_position_checks'],
        ambiguous_recovery_receipts=audit['ambiguous_recovery_receipts'],
        ew_eligible_radar_member_checks=audit['ew_checks'], ew_effective_checks=audit['effective_ew_checks'], sides=sides))

repeat = None
if len(a.matches) > 1 and 'repeat' in a.matches[-1].name:
    first = json.loads(a.matches[0].read_text(encoding='utf-8-sig'))
    last = json.loads(a.matches[-1].read_text(encoding='utf-8-sig'))
    first.pop('wall_seconds', None)
    last.pop('wall_seconds', None)
    repeat = first == last
    assert repeat, 'same-seed diagnostic output changed beyond wall time'

result = dict(matches=rows, exact_repeat_except_wall_time=repeat, limits=[
    'Per-airframe strike rates use initial mission at birth and include late departures still airborne at end.',
    'Fresh-track seconds are per aircraft, not independent package-seconds; no hidden position is supplied to AI.',
    'Recovery attribution requires same tick/class/home/proximity and exact count; ambiguous receipts remain separate.',
    'Release envelope is a measured opportunity, not proof of an accepted command or hit.',
    'Fixed minute terminal bins are descriptive; boundary effects and accidental coincidence remain.',
    'EW checks use the existing combined noise modifier on an emitting radar; not marginal kill attribution.',
    'These native runs are FSM versus FSM, not an LLM match.'
])
a.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps([dict(file=r['file'], score=[r['result']['blue_points'], r['result']['red_points']],
    air_asm={s:v['actual_air_asm_rounds'] for s,v in r['sides'].items()}) for r in rows]))
