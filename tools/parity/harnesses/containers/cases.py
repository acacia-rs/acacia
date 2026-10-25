#!/usr/bin/env python3
"""Random container screen cases, one generator per kind, drawn round-robin; one JSON case per line.

cases.py [--count N] [--kind K]...

A case: kind, GUI size, mouse, partial tick, title (component JSON or null for the default), the
menu's slot contents ([index, stack]), the carried stack, a quick-craft drag in progress, plus the
kind's own state. Stacks are ItemStack.CODEC JSON.
"""
import argparse
import json
import random

SIZES = [(427, 240), (320, 240), (480, 270), (640, 360), (961, 501), (176, 166), (200, 180), (379, 250), (378, 250)]
# (id, max stack) of items whose tooltips are a name and little else.
PLAIN = [('minecraft:stone', 64), ('minecraft:dirt', 64), ('minecraft:apple', 64), ('minecraft:oak_log', 64),
         ('minecraft:torch', 64), ('minecraft:ender_pearl', 16), ('minecraft:snowball', 16), ('minecraft:bucket', 16),
         ('minecraft:saddle', 1), ('minecraft:cake', 1), ('minecraft:golden_apple', 64), ('minecraft:nether_star', 64),
         ('minecraft:white_wool', 64), ('minecraft:glass', 64), ('minecraft:bread', 64), ('minecraft:coal', 64)]
TITLES = [None, None, None, {'text': 'Loot'}, {'text': 'A very long custom container title that overflows'},
          {'text': 'Red', 'color': 'red'}, {'translate': 'container.enderchest'}, {'text': ''}]

# kind: (image width, image height, menu slots)
KINDS = {
    'chest': None,
    'shulker': (176, 167, 27 + 36),
    'dispenser': (176, 166, 9 + 36),
    'hopper': (176, 133, 5 + 36),
    'brewing': (176, 166, 5 + 36),
    'crafter': (176, 166, 9 + 36 + 1),
}


def data(rng, kind):
    """The menu's data slots (ContainerData), by kind."""
    if kind == 'brewing':
        total = rng.choice([400, 400, 20, 1, 0])
        fuel_total = rng.choice([20, 20, 7, 0])
        return [rng.choice([0, 0, rng.randint(0, max(total, 1)), total, 1, 2, 13]), rng.randint(0, max(fuel_total, 1)), total, fuel_total]
    if kind == 'crafter':
        return [int(rng.random() < 0.3) for _ in range(9)] + [int(rng.random() < 0.5)]
    return None


def stack(rng, sizes=True):
    item, most = rng.choice(PLAIN)
    count = rng.choice([1, 1, most, rng.randint(1, most)]) if sizes else 1
    s = {'id': item, 'count': count}
    if rng.random() < 0.1:
        s['components'] = {'minecraft:custom_name': rng.choice(['Named', {'text': 'Fancy', 'color': 'gold'}])}
    return s


def base(rng, kind, image, slots):
    width, height = rng.choice(SIZES)
    left, top = (width - image[0]) // 2, (height - image[1]) // 2
    if rng.random() < 0.8:
        mouse = [rng.randint(left - 4, left + image[0] + 4), rng.randint(top - 4, top + image[1] + 4)]
    else:
        mouse = [rng.randint(0, width), rng.randint(0, height)]
    case = {'kind': kind, 'width': width, 'height': height, 'mouse': mouse, 'partial': round(rng.random(), 3),
            'title': rng.choice(TITLES), 'advanced': rng.random() < 0.2,
            'slots': [[i, stack(rng)] for i in range(slots) if rng.random() < rng.choice([0.0, 0.3, 0.6, 0.9])]}
    if rng.random() < 0.25:
        case['carried'] = stack(rng)
        if rng.random() < 0.4:
            case['quick'] = {'type': rng.choice([0, 1, 2]), 'slots': rng.sample(range(slots), rng.randint(1, 6))}
    return case


def make(kind):
    def gen(rng):
        if kind == 'chest':
            rows = rng.randint(1, 6)
            case = base(rng, kind, (176, 114 + rows * 18), rows * 9 + 36)
            case['rows'] = rows
            return case
        w, h, slots = KINDS[kind]
        case = base(rng, kind, (w, h), slots)
        values = data(rng, kind)
        if values is not None:
            case['data'] = values
        return case
    return gen


GENERATORS = {k: make(k) for k in KINDS}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--count', type=int, default=2400)
    parser.add_argument('--kind', action='append', choices=sorted(GENERATORS))
    parser.add_argument('--seed', type=int, default=0x636f6e74)
    args = parser.parse_args()
    kinds = args.kind or list(GENERATORS)
    rng = random.Random(args.seed)
    for n in range(args.count):
        print(json.dumps(GENERATORS[kinds[n % len(kinds)]](rng), separators=(',', ':')))


if __name__ == '__main__':
    main()
