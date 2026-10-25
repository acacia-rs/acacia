#!/usr/bin/env python3
"""Random tooltip cases, one per line: item tooltips by area (kinds.py) and plain `lines` tooltips.

cases.py [--count N] [--kind K]...

An item case: kind (its area), GUI size, mouse (edges often), the stack, the advanced/creative/op
flags, a peaceful level and map data ([id, scale, locked]). A `lines` case: tooltips set in order,
each {lines (component JSON), x, y, positioner [type, x, y, w, h] (default, below or menu), style,
replace}. Cases for tooltip_display's flags adapted from Horizon's tools/test_tooltip_display_parity.py
(MIT, (c) Ludovic Marie).
"""
import argparse
import json
import random

from kinds import AREAS
from stacks import STYLED, TEXTS

SIZES = [(427, 240), (320, 240), (480, 270), (640, 360), (961, 501), (176, 166), (200, 120), (100, 80), (854, 480)]


def mouse(rng, width, height):
    r = rng.random()
    if r < 0.15:
        return [rng.choice([0, 1, width - 1, width]), rng.randint(0, height)]
    if r < 0.3:
        return [rng.randint(0, width), rng.choice([0, 1, 12, height - 1, height])]
    return [rng.randint(0, width), rng.randint(0, height)]


def item(area):
    def gen(rng):
        width, height = rng.choice(SIZES)
        case = {'kind': area, 'width': width, 'height': height, 'mouse': mouse(rng, width, height),
                'stack': AREAS[area](rng), 'advanced': rng.random() < 0.3, 'creative': rng.random() < 0.2,
                'op': rng.random() < 0.2}
        if rng.random() < 0.1:
            case['peaceful'] = True
        if area == 'gadgets':
            case['maps'] = [[i, rng.randint(0, 4), rng.random() < 0.4] for i in (0, 1, 2) if rng.random() < 0.7]
        return case
    return gen


def line(rng):
    r = rng.random()
    if r < 0.6:
        return rng.choice(TEXTS)
    if r < 0.8:
        return rng.choice(STYLED)
    return {'text': rng.choice(TEXTS), 'color': rng.choice(['red', 'gray', '#FF00FF', 'gold'])}


def lines(rng):
    width, height = rng.choice(SIZES)
    tooltips = []
    for _ in range(rng.choice([1, 1, 1, 2, 3])):
        t = {'lines': [line(rng) for _ in range(rng.choice([0, 1, 1, 2, 3, 5, 12, 30]))], 'x': rng.randint(-5, width + 5),
             'y': rng.randint(-5, height + 5)}
        if rng.random() < 0.6:
            w, h = rng.randint(1, 200), rng.randint(1, 40)
            t['positioner'] = [rng.choice(['default', 'below', 'menu']), rng.randint(-10, width), rng.randint(-10, height), w, h]
        if rng.random() < 0.2:
            t['style'] = rng.choice(['minecraft:fancy', 'test:other'])
        if rng.random() < 0.3:
            t['replace'] = True
        tooltips.append(t)
    return {'kind': 'lines', 'width': width, 'height': height, 'mouse': mouse(rng, width, height), 'tooltips': tooltips}


GENERATORS = {area: item(area) for area in AREAS}
GENERATORS['lines'] = lines


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--count', type=int, default=5200)
    parser.add_argument('--kind', action='append', choices=sorted(GENERATORS))
    parser.add_argument('--seed', type=int, default=0x746f6f6c)
    args = parser.parse_args()
    kinds = args.kind or list(GENERATORS)
    rng = random.Random(args.seed)
    for n in range(args.count):
        print(json.dumps(GENERATORS[kinds[n % len(kinds)]](rng), separators=(',', ':')))


if __name__ == '__main__':
    main()
