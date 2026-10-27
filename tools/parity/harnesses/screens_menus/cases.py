#!/usr/bin/env python3
"""Random menu-screen cases, one generator per screen kind, drawn round-robin; one JSON case per line.

cases.py [--count N] [--kind K]... [--seed S]
Every case has the GUI size, the mouse (half the time over a likely button row), the menu blur
(0 turns the blur call off) and the ticks run after init (buttons that wake after a delay).
"""
import argparse
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'gui'))
from screencases import message, mouse, size, words  # noqa: E402


def base(rng):
    case = {**size(rng), 'blur': rng.choice([5, 5, 0, 1, 10]), 'ticks': rng.choice([0, 0, 1, 19, 20, 21, 40])}
    case['mouse'] = mouse(rng, case)
    if rng.random() < 0.5:
        case['mouse'] = [case['width'] // 2 + rng.randint(-110, 110), case['height'] // 4 + rng.randint(-20, 130)]
    return case


def pause(rng):
    return {**base(rng), 'showMenu': rng.random() < 0.85, 'topmost': rng.random() < 0.8, 'localServer': rng.random() < 0.5}


def death(rng):
    case = {**base(rng), 'hardcore': rng.random() < 0.3, 'score': rng.choice([0, 7, 123, 99999, -4])}
    if rng.random() < 0.7:
        case['cause'] = rng.choice([{'translate': 'death.attack.generic', 'with': ['Steve']}, message(rng, 2, 12),
                                    {'translate': 'death.fell.accident.generic', 'with': [{'text': 'Alex', 'color': 'gold'}]}])
    return case


def confirm(rng):
    case = {**base(rng), 'title': message(rng, 1, 5), 'message': message(rng, 0, 60), 'yes': rng.choice([{'translate': 'gui.yes'}, message(rng)]),
            'no': rng.choice([{'translate': 'gui.no'}, message(rng)]), 'deathBackground': rng.random() < 0.2}
    if rng.random() < 0.4:
        case['delay'] = rng.choice([0, 10, 20])
    return case


def alert(rng):
    return {**base(rng), 'title': message(rng, 1, 5), 'message': message(rng, 0, 50), 'ok': rng.choice([{'translate': 'gui.back'}, message(rng)])}


def progress(rng):
    case = {**base(rng), 'progress': rng.choice([0, 0, 1, 50, 100])}
    if rng.random() < 0.8:
        case['header'] = message(rng, 1, 6)
    if rng.random() < 0.7:
        case['stage'] = rng.choice([{'translate': 'menu.working'}, message(rng, 1, 4)])
    return case


def disconnected(rng):
    return {**base(rng), 'title': rng.choice([{'translate': 'connect.failed'}, message(rng, 1, 4)]),
            'message': rng.choice([{'translate': 'disconnect.lost'}, message(rng, 0, 50), words(rng, 20, 60)])}


def completed(rng):
    return {**base(rng), 'title': message(rng, 1, 5), 'message': message(rng, 0, 60), 'button': rng.choice([{'translate': 'gui.done'}, message(rng)])}


KINDS = dict(pause=pause, death=death, confirm=confirm, alert=alert, progress=progress, disconnected=disconnected, completed=completed)


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
