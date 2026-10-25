#!/usr/bin/env python3
"""Compares a GUI oracle's draw calls with acacia's, case by case.

compare.py VANILLA RUST [--detail N]

Each line of VANILLA is {case, draws, <state>...} or {case, error}; the same line of RUST is
{draws, <state>...}. A case matches when its ordered call lists agree field by field (floats to
1e-4 relative) and every state key the oracle read back agrees. Prints the first N mismatches
(where the lists part, both sides' next calls), a match count per case kind, and exits 1 on any
mismatch or oracle error. Adapted from Horizon's test_hud_parity.py (MIT, (c) Ludovic Marie).
"""
import argparse
import json
import sys

IDENTITY = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]


def close(p, q):
    if isinstance(p, bool) or isinstance(q, bool):
        return p == q
    if isinstance(p, (int, float)) and isinstance(q, (int, float)):
        return abs(p - q) <= 1e-4 * (1 + abs(p))
    if isinstance(p, list) and isinstance(q, list):
        return len(p) == len(q) and all(close(a, b) for a, b in zip(p, q))
    if isinstance(p, dict) and isinstance(q, dict):
        return p.keys() == q.keys() and all(close(p[k], q[k]) for k in p)
    return p == q


def normal(d):
    # Gson leaves out null members, so a null field and a missing one are the same.
    d = {k: v for k, v in d.items() if v is not None}
    if d['type'] != 'item':
        d.setdefault('matrix', IDENTITY)
    if d['type'] == 'sprite':
        d.setdefault('pipeline', 'pipeline/gui_textured')
    if d['type'] == 'fill':
        d.setdefault('pipeline', 'pipeline/gui')
    return d


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('vanilla')
    parser.add_argument('rust')
    parser.add_argument('--detail', type=int, default=3)
    args = parser.parse_args()
    vanilla = [json.loads(l) for l in open(args.vanilla, encoding='utf-8') if l.strip()]
    rust = [json.loads(l) for l in open(args.rust, encoding='utf-8') if l.strip()]
    if len(vanilla) != len(rust):
        sys.exit(f'{len(vanilla)} oracle lines, {len(rust)} probe lines')
    errors, bad, totals = [], [], {}
    for v, r in zip(vanilla, rust):
        kind = v['case'].get('kind', '-')
        totals.setdefault(kind, [0, 0])[1] += 1
        if 'error' in v:
            errors.append(v['error'])
            continue
        a = [normal(d) for d in v['draws']]
        b = [normal(d) for d in r['draws']]
        for key in sorted(v.keys() - {'case', 'draws'}):
            if key not in r or not close(v[key], r[key]):
                a.append({'type': 'state', key: v[key]})
                b.append({'type': 'state', key: r.get(key)})
        if len(a) != len(b) or not all(close(x, y) for x, y in zip(a, b)):
            first = next((i for i, (x, y) in enumerate(zip(a, b)) if not close(x, y)), min(len(a), len(b)))
            bad.append((v['case'], first, a[first:first + 2], b[first:first + 2], len(a), len(b)))
        else:
            totals[kind][0] += 1
    for e in sorted(set(errors))[:5]:
        print('ORACLE ERROR', e)
    for case, i, a, b, la, lb in bad[:args.detail]:
        print('MISMATCH', json.dumps(case, ensure_ascii=False)[:2000])
        print(f'   at call {i} ({la} game, {lb} acacia calls)\n   game   {a}\n   acacia {b}')
    for kind, (ok, n) in sorted(totals.items()):
        print(f'  {kind:14} {ok}/{n}')
    print(f'{len(vanilla) - len(bad) - len(errors)}/{len(vanilla)} cases match; {len(bad)} divergent; {len(errors)} oracle errors')
    sys.exit(1 if bad or errors else 0)


if __name__ == '__main__':
    main()
