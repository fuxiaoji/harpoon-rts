"""Summarize the original seven N1 prefixes without turning them into win rates."""
import argparse
from collections import Counter
import json
from pathlib import Path
import statistics


def read(path):
    return json.loads(path.read_text(encoding='utf-8'))


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--stage',type=Path,required=True)
    stage=parser.parse_args().stage.resolve()
    rows=[]; elapsed=[]; sim_delay=[]; usage=Counter(); statuses=Counter()
    for directory in sorted((stage/'live-pilot').iterdir()):
        if not (directory/'protocol.json').exists():continue
        summary=read(directory/'summary.json')
        requests=read(directory/'requests.json')
        dispatches=read(directory/'dispatch-ledger.json')
        assert [r['id'] for r in requests]==[r['id'] for r in dispatches]
        assert len(requests)==summary['requests']<=3
        local_usage=Counter(); known=0
        for request in requests:
            receipt=read(directory/'calls'/request['id']/'result.json')
            assert receipt.get('usage')==request['usage']
            if request['usage'] is not None:
                known+=1;local_usage.update(request['usage']);usage.update(request['usage'])
            statuses[request['status']]+=1
            if request['status']=='ok':
                elapsed.append(request['elapsed_s'])
                if request['received_tick'] is not None:
                    sim_delay.append(request['received_tick']-request['request_tick'])
        ticks=[r['request_tick'] for r in requests]
        rows.append({'policy':summary['protocol']['policy'],'tick':summary['tick'],
            'terminal':summary['over'],'result':summary['result'],
            'client_starts':len(dispatches),'valid_receipts':summary['valid_receipts'],
            'usable_receipts':summary['usable_receipts'],'usage_known':known,
            'usage_unknown':summary['usage_unknown'],'usage_sums_known_only':dict(local_usage),
            'dispatch_ticks':ticks,'actual_dispatch_gaps_s':[b-a for a,b in zip(ticks,ticks[1:])],
            'receipt_ticks':[r['received_tick'] for r in requests],
            'commands_proposed':sum(r['commands_proposed'] for r in requests),
            'commands_passed_validation':sum(r.get('commands_passed_validation',0) for r in requests),
            'terminal_inflight':summary['terminal_inflight'],
            'call_cap_reached':summary['call_cap_reached'],
            'max_clock_lag_s':summary['max_clock_lag_s'],
            'confirmed_points_at_prefix':summary['own_victory']['own_kill_points'],
            'own_losses_at_prefix':summary['own_victory']['own_losses']})
    assert len(rows)==7 and len({r['policy'] for r in rows})==7
    assert all(r['tick']==1200 and not r['terminal'] and r['result'] is None for r in rows)
    assert sum(r['client_starts'] for r in rows)<=21
    output={'scope':'single-seed real-clock interface prefixes, not complete-game win rates',
        'rows':rows,'client_starts':sum(r['client_starts'] for r in rows),
        'valid_receipts':sum(r['valid_receipts'] for r in rows),
        'usable_receipts':sum(r['usable_receipts'] for r in rows),
        'usage_unknown':sum(r['usage_unknown'] for r in rows),'statuses':dict(statuses),
        'usage_sums_known_only':dict(usage),
        'token_accounting':'cached_input is a subset of input; reasoning_output is a subset of output; do not sum these columns',
        'successful_elapsed_wall_s':{'median':statistics.median(elapsed),'min':min(elapsed),'max':max(elapsed)},
        'successful_delivery_delay_sim_s':{'median':statistics.median(sim_delay),'min':min(sim_delay),'max':max(sim_delay)},
        'max_clock_lag_s':max(r['max_clock_lag_s'] for r in rows),
        'limits':['F300 has only preserved local startup failures; it is not a successful model policy',
                  'original and resumed arms ran under different process environments and time blocks',
                  'call ceiling binds; seven directories are not seven independent task families',
                  'no monetary usage or verified output-token hard cap',
                  'plan-age installation bug requires the separately archived differential audit']}
    (stage/'live-pilot-analysis.json').write_text(json.dumps(output,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({k:v for k,v in output.items() if k not in ('rows','limits')},ensure_ascii=False))


if __name__=='__main__':main()
