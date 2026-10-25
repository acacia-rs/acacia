#!/usr/bin/env python3
"""Random text-screen cases (sign, book_view, book_edit, telemetry), drawn round-robin; one JSON case per line.

cases.py [--count N] [--kind K]... [--seed S]
Every case has the GUI size, the mouse (often over a button or page arrow) and the menu blur.
sign: wood, hanging, wall, dye, glowing, 4 lines, line, cursor, selection, blinkAgo.
book_view: pages (components), page. book_edit: pages (strings), page, ops run on the page's
field ([seek, pos, selecting] | [insert, s] | [delete, dir] | [line, +-1] | [point, x, y]),
focused, blinkAgo. telemetry: extra (EXTRA_TELEMETRY_AVAILABLE), optIn, scroll, focused.
blinkAgo sits mid-phase (300 ms phases) so the oracle's clock drift never flips the caret.
"""
import argparse
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'gui'))
from screencases import WORDS, message, mouse, size, words  # noqa: E402

WOODS = ['oak', 'spruce', 'birch', 'acacia', 'cherry', 'jungle', 'dark_oak', 'pale_oak', 'crimson', 'warped', 'mangrove', 'bamboo']
DYES = ['white', 'orange', 'magenta', 'light_blue', 'yellow', 'lime', 'pink', 'gray', 'light_gray', 'cyan', 'purple', 'blue',
        'brown', 'green', 'red', 'black']
BLINK = [150, 450, 750, 1050, 1350, 100, 520]


def base(rng):
    case = {**size(rng), 'blur': rng.choice([5, 5, 0, 1])}
    case['mouse'] = mouse(rng, case)
    return case


def near(rng, x, y, w, h):
    return [x + rng.randint(-3, w + 3), y + rng.randint(-3, h + 3)]


def fit(rng, n):
    return words(rng, 0, 3)[:n]


def sign(rng):
    case = base(rng)
    hanging = rng.random() < 0.4
    lines = [fit(rng, 8 if hanging else 13) for _ in range(4)]
    line = rng.randrange(4)
    n = len(lines[line])
    cursor = rng.choice([n, n, rng.randint(0, n), 0])
    selection = cursor if rng.random() < 0.6 else rng.randint(0, n)
    if rng.random() < 0.3:
        case['mouse'] = near(rng, case['width'] // 2 - 100, case['height'] // 4 + 144, 200, 20)
    return {**case, 'wood': rng.choice(WOODS), 'hanging': hanging, 'wall': rng.random() < 0.4, 'dye': rng.choice(DYES),
            'glowing': rng.random() < 0.35, 'lines': lines, 'line': line, 'cursor': cursor, 'selection': selection,
            'blinkAgo': rng.choice(BLINK)}


def arrows(rng, case):
    left = (case['width'] - 192) // 2
    r = rng.random()
    if r < 0.25:
        case['mouse'] = near(rng, left + 43, 159, 23, 13)
    elif r < 0.5:
        case['mouse'] = near(rng, left + 116, 159, 23, 13)
    elif r < 0.65:
        case['mouse'] = near(rng, case['width'] // 2 - 100, 196, 200, 20)


def page_component(rng):
    r = rng.random()
    if r < 0.3:
        return words(rng, 0, 80)
    if r < 0.4:
        return 'WWWWWWWWWWWWWWWWWWWWWWWWWWWWWW ' + words(rng, 0, 10)
    if r < 0.5:
        return {'text': words(rng, 1, 30) + '\n\n' + words(rng, 0, 20), 'color': 'dark_blue'}
    return [message(rng, 0, 20), {'text': ' ' + words(rng, 0, 40), 'bold': rng.random() < 0.3, 'underlined': rng.random() < 0.3}]


def book_view(rng):
    case = base(rng)
    n = rng.choice([0, 1, 1, 2, 3, 5, 8])
    pages = [page_component(rng) for _ in range(n)]
    page = 0 if n < 2 else rng.choice([0, n - 1, rng.randrange(n)])
    arrows(rng, case)
    return {**case, 'pages': pages, 'page': page}


def page_text(rng):
    r = rng.random()
    if r < 0.15:
        return ''
    if r < 0.3:
        return 'WWWWWWWWWWWWWWWWWWWWWWWWWWWWWW' + words(rng, 0, 10)
    if r < 0.5:
        return '\n'.join(words(rng, 0, 6) for _ in range(rng.randint(1, 8))) + rng.choice(['', '\n'])
    if r < 0.6:
        return words(rng, 120, 200)
    return words(rng, 1, 70)


def ops(rng, value):
    out = []
    length = len(value)
    for _ in range(rng.choice([0, 1, 1, 2, 3, 5])):
        r = rng.random()
        if r < 0.35:
            out.append(['seek', rng.randint(0, length + 2), rng.random() < 0.4])
        elif r < 0.55:
            s = rng.choice([rng.choice(WORDS), ' ', '\n', words(rng, 1, 8), 'WWWWWWWWWWWWWWWWWWWWWWW'])
            out.append(['insert', s])
            length += len(s)
        elif r < 0.65:
            out.append(['delete', rng.choice([-1, 1])])
        elif r < 0.8:
            out.append(['line', rng.choice([-1, 1])])
        else:
            out.append(['point', round(rng.uniform(-5, 125), 2), round(rng.uniform(-5, 150), 2)])
    return out


def book_edit(rng):
    case = base(rng)
    n = rng.choice([0, 1, 1, 2, 3, 6])
    pages = [page_text(rng) for _ in range(n)]
    page = 0 if n < 2 else rng.choice([0, n - 1, rng.randrange(n)])
    arrows(rng, case)
    return {**case, 'pages': pages, 'page': page, 'ops': ops(rng, pages[page] if pages else ''), 'focused': rng.random() < 0.85,
            'blinkAgo': rng.choice(BLINK)}


def telemetry(rng):
    case = base(rng)
    if rng.random() < 0.3:
        case['mouse'] = [case['width'] // 2 + rng.randint(-160, 160), case['height'] - rng.randint(0, 60)]
    return {**case, 'extra': rng.random() < 0.6, 'optIn': rng.random() < 0.5, 'focused': rng.random() < 0.8,
            'scroll': rng.choice([0, 0, 9, 100, 450.5, 1000, 5000, rng.uniform(0, 1500)])}


KINDS = dict(sign=sign, book_view=book_view, book_edit=book_edit, telemetry=telemetry)


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
