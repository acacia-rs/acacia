#!/usr/bin/env python3
"""Random loading, message, credits and win-screen cases, drawn round-robin; one JSON case per line.

cases.py [--count N] [--kind K]... [--seed S]
Every case has the GUI size, the mouse, the menu blur (0 turns the blur call off) and whether a
level is loaded (in-world menu textures, no panorama on the default background).
loading: `reason`, `hasProgress`, `progress` (the server's, eased in over `ticks`), an optional
chunk `view` (`radius`, `cells` x major: a status index or -1) and the `visualize` debug mark.
win: `poem`, `scroll`, the frame's `partial` tick, `speedup`, `modifiers` (Ctrl keys held),
`reverse` (Up held) and the `player` named in the poem.
"""
import argparse
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'gui'))
from screencases import message, mouse, size  # noqa: E402


def base(rng):
    case = {**size(rng), 'blur': rng.choice([5, 5, 0, 1, 10]), 'inWorld': rng.random() < 0.6}
    case['mouse'] = mouse(rng, case)
    if rng.random() < 0.5:
        case['mouse'] = [case['width'] // 2 + rng.randint(-110, 110), case['height'] // 6 + rng.randint(0, 120)]
    return case


def loading(rng):
    case = {**base(rng), 'reason': rng.choice(['OTHER', 'OTHER', 'NETHER_PORTAL', 'END_PORTAL']), 'hasProgress': rng.random() < 0.7,
            'progress': rng.choice([0.0, 1.0, 0.5, round(rng.random(), 3), rng.random()]), 'ticks': rng.choice([0, 1, 3, 10, 40]),
            'visualize': rng.random() < 0.3}
    if rng.random() < 0.6:
        radius = rng.choice([0, 1, 3, 5, 11, 12])
        fill = rng.random()
        case['view'] = {'radius': radius, 'cells': [rng.randint(0, 9) if rng.random() < fill else -1 for _ in range((radius * 2 + 1) ** 2)]}
    return case


def generic(rng):
    return {**base(rng), 'title': rng.choice([{'translate': 'connect.joining'}, {'translate': 'menu.savingLevel'}, message(rng, 1, 12)])}


def credits(rng):
    return base(rng)


def win(rng):
    case = {**base(rng), 'poem': rng.random() < 0.4, 'partial': rng.choice([0.0, 1.0, round(rng.random(), 3)]),
            'speedup': rng.random() < 0.3, 'modifiers': rng.choice([0, 0, 1, 2]), 'reverse': rng.random() < 0.15,
            'player': rng.choice(['Steve', 'Alex', 'xX_long_name_Xx', 'a'])}
    # Through the whole roll: the logo, the poem, every credits section and the end.
    case['scroll'] = rng.choice([0.0, 30.5, rng.uniform(0, 400), rng.uniform(0, 30000), rng.uniform(30000, 60000), 1e6])
    return case


KINDS = dict(loading=loading, message=generic, credits=credits, win=win)


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--count', type=int, default=2400)
    p.add_argument('--kind', action='append')
    p.add_argument('--seed', type=int, default=1)
    a = p.parse_args()
    rng = random.Random(a.seed)
    kinds = a.kind or list(KINDS)
    for n in range(a.count):
        kind = kinds[n % len(kinds)]
        print(json.dumps({'kind': kind, **KINDS[kind](rng)}))


if __name__ == '__main__':
    main()
