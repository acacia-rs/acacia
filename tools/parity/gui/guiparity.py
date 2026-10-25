#!/usr/bin/env python3
"""The GUI harnesses' shared run and comparison: draw-call lists, the game's against acacia's.

A GUI harness (tools/parity/harnesses/<name>/) has `cases.py` (prints one JSON case per line,
each with a `kind`), an oracle `main(outDir, casesFile)` writing `vanilla.jsonl` (lines
{case, draws, <state>...} or {case, error}) and a module of `tools/parity/gui-probe` writing the
same lines (without `case`). A case matches when its ordered call lists agree field by field
(numbers within 1e-4 relative) and every state key the oracle read back agrees.

  guiparity.py VANILLA RUST [--detail N]   compares two runs by hand
Adapted from Horizon's test_hud_parity.py (MIT, (c) Ludovic Marie).
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

IDENTITY = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
ASSETS = Path(os.environ.get("GUI_ASSETS", Path.home() / "gui-assets" / "assets" / "minecraft"))


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


def compare(vanilla, rust):
    """Returns (cases, divergences as (case, field, expected, got), {kind: [matched, total]})."""
    vanilla = [json.loads(l) for l in open(vanilla, encoding='utf-8') if l.strip()]
    rust = [json.loads(l) for l in open(rust, encoding='utf-8') if l.strip()]
    if len(vanilla) != len(rust):
        raise RuntimeError(f'{len(vanilla)} oracle lines, {len(rust)} probe lines')
    divergences, totals = [], {}
    for n, (v, r) in enumerate(zip(vanilla, rust)):
        kind = v['case'].get('kind', '-')
        name = f'{kind}#{n}'
        totals.setdefault(kind, [0, 0])[1] += 1
        if 'error' in v:
            divergences.append((name, 'oracle error', v['error'], ''))
            continue
        a = [normal(d) for d in v['draws']]
        b = [normal(d) for d in r['draws']]
        for key in sorted(v.keys() - {'case', 'draws'}):
            if key not in r or not close(v[key], r[key]):
                a.append({'type': 'state', key: v[key]})
                b.append({'type': 'state', key: r.get(key)})
        if len(a) == len(b) and all(close(x, y) for x, y in zip(a, b)):
            totals[kind][0] += 1
            continue
        i = next((i for i, (x, y) in enumerate(zip(a, b)) if not close(x, y)), min(len(a), len(b)))
        field = f'call {i} of {len(a)} (acacia: {len(b)}); case {json.dumps(v["case"], ensure_ascii=False)[:600]}'
        divergences.append((name, field, json.dumps(a[i:i + 2], ensure_ascii=False), json.dumps(b[i:i + 2], ensure_ascii=False)))
    return len(vanilla), divergences, totals


def cases(harness_dir, out, args):
    """Runs the harness's cases.py into `out`/inputs.jsonl."""
    path = Path(out) / 'inputs.jsonl'
    with open(path, 'w', encoding='utf-8') as f:
        subprocess.run([sys.executable, Path(harness_dir) / 'cases.py', *map(str, args)], stdout=f, check=True)
    return path


def assets():
    """The client jar's assets/minecraft, unpacked once beside the oracle home."""
    if not ASSETS.is_dir():
        from paritylib.home import GAME
        root = ASSETS.parents[1]
        root.mkdir(parents=True, exist_ok=True)
        subprocess.run(['unzip', '-q', '-o', GAME / 'client.jar', 'assets/*', '-d', root], check=True)
    return ASSETS


def run(h, main, *case_args):
    """A GUI harness's `run(h)` (docs/parity.md): cases, oracle, probe, comparison. GUI_CASES
    (e.g. `--count 300 --kind chat`) replaces the harness's case arguments while iterating."""
    if os.environ.get('GUI_CASES'):
        case_args = os.environ['GUI_CASES'].split()
    inputs = cases(h.dir, h.out, case_args)
    expected = h.oracle(main, inputs, inputs=[inputs])
    rust = h.out / 'rust.jsonl'
    h.cargo('run', '--release', '-q', '-p', 'acacia-gui-probe', '--', h.name, assets(), inputs, rust)
    n, divergences, totals = compare(expected / 'vanilla.jsonl', rust)
    for kind, (ok, total) in sorted(totals.items()):
        h.log.write(f'  {kind:14} {ok}/{total}\n')
    h.add(n, divergences)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('vanilla')
    parser.add_argument('rust')
    parser.add_argument('--detail', type=int, default=3)
    args = parser.parse_args()
    n, divergences, totals = compare(args.vanilla, args.rust)
    for case, field, expected, got in divergences[:args.detail]:
        print(f'MISMATCH {case} at {field}\n   game   {expected}\n   acacia {got}')
    for kind, (ok, total) in sorted(totals.items()):
        print(f'  {kind:14} {ok}/{total}')
    print(f'{n - len(divergences)}/{n} cases match; {len(divergences)} divergent')
    sys.exit(1 if divergences else 0)


if __name__ == '__main__':
    main()
