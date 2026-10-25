#!/usr/bin/env python3
"""Random creative inventory cases, one generator per kind of selected tab, drawn round-robin.

cases.py [--count N] [--kind K]... [--seed S]

A case: kind, tab (creative tab id), GUI size, mouse, partial tick, advanced tooltips, the player's
operator permissions (op) and the screen's operator-tab option (opTab), the player's inventory
([inventory slot, stack]), the carried stack, active effects, the scroll offset, plus per kind: the
search box (text, cursor, highlight, blink phase), the saved hotbars (9 rows of 9 stacks; every
row holds something, as empty rows show key names the stand-in client cannot resolve) or the
player figure (bounding box, scale, fall flying). Stacks are ItemStack.CODEC JSON. Cases are
ordered by whether the operator tab shows, as the tabs are rebuilt on each change.
"""
import argparse
import json
import random

SIZES = [(427, 240), (320, 240), (480, 270), (640, 360), (961, 501), (200, 180), (379, 250), (378, 250), (195, 136)]
PLAIN = [('minecraft:stone', 64), ('minecraft:dirt', 64), ('minecraft:apple', 64), ('minecraft:oak_log', 64),
         ('minecraft:torch', 64), ('minecraft:ender_pearl', 16), ('minecraft:snowball', 16), ('minecraft:bucket', 16),
         ('minecraft:saddle', 1), ('minecraft:cake', 1), ('minecraft:golden_apple', 64), ('minecraft:nether_star', 64),
         ('minecraft:white_wool', 64), ('minecraft:glass', 64), ('minecraft:bread', 64), ('minecraft:coal', 64)]
CATEGORIES = ['building_blocks', 'colored_blocks', 'natural_blocks', 'functional_blocks', 'redstone_blocks',
              'tools_and_utilities', 'combat', 'food_and_drinks', 'ingredients', 'spawn_eggs', 'op_blocks']
QUERIES = ['', 'a', 'stone', 'oak', 'Red', 'wool', 'xyz', 'diamond sword', 'minecraft:', 'minecraft:dirt', 'egg',
           '#logs', '#minecraft:wool', '#sword', '#', 'enchanted book', 'light gray stained glass pane',
           'a very long search that scrolls past the box', 'WWWWWWWWWWWWWWWWWWWW', 'iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii',
           'potion of', 'spawn', ' ', 'b', 'Stone Bricks']
EFFECTS = ['speed', 'haste', 'strength', 'regeneration', 'fire_resistance', 'night_vision', 'invisibility',
           'water_breathing', 'luck', 'slow_falling', 'conduit_power', 'hero_of_the_village', 'absorption']


def stack(rng):
    item, most = rng.choice(PLAIN)
    s = {'id': item, 'count': rng.choice([1, 1, most, rng.randint(1, most)])}
    if rng.random() < 0.1:
        s['components'] = {'minecraft:custom_name': rng.choice(['Named', {'text': 'Fancy', 'color': 'gold'}])}
    return s


def mouse(rng, width, height, kind):
    left, top = (width - 195) // 2, (height - 136) // 2
    r = rng.random()
    if r < 0.4:
        if kind == 'inventory':
            x, y = rng.choice([(9 + rng.randint(0, 8) * 18, rng.choice([54, 72, 90, 112])), (54, 6), (54, 33), (108, 6),
                               (108, 33), (35, 20), (173, 112), (80, 20)])
        else:
            x, y = 9 + rng.randint(0, 8) * 18, 18 + rng.randint(0, 5) * 18
            y = 112 if y == 108 else y
        return [left + x + rng.randint(-2, 17), top + y + rng.randint(-2, 17)]
    if r < 0.7:
        column = rng.randint(0, 6)
        x = rng.choice([27 * column, 195 - 27 * (7 - column) + 1])
        y = rng.choice([-32, 136])
        return [left + x + rng.randint(0, 28), top + y + rng.randint(0, 34)]
    if r < 0.8:
        return [left + rng.randint(170, 192), top + rng.randint(14, 134)]
    return [rng.randint(0, width), rng.randint(0, height)]


def base(rng, kind, tab):
    width, height = rng.choice(SIZES)
    case = {'kind': kind, 'tab': 'minecraft:' + tab, 'width': width, 'height': height,
            'mouse': mouse(rng, width, height, kind), 'partial': round(rng.random(), 3),
            'advanced': rng.random() < 0.2, 'op': rng.random() < 0.5, 'opTab': rng.random() < 0.5,
            'inventory': [[i, stack(rng)] for i in range(41) if rng.random() < rng.choice([0.0, 0.2, 0.5, 0.9])]}
    if rng.random() < 0.15:
        case['carried'] = stack(rng)
    if rng.random() < 0.15:
        case['effects'] = [['minecraft:' + e, rng.choice([20, 600, 3600, -1, 72000]), rng.choice([0, 1, 4]), rng.random() < 0.2]
                           for e in rng.sample(EFFECTS, rng.randint(1, 6))]
    if rng.random() < 0.7:
        case['scroll'] = rng.choice([0.0, 1.0, 0.5, round(rng.random(), 4), round(rng.random(), 4)])
    return case


def category(rng):
    return base(rng, 'category', rng.choice(CATEGORIES))


def inventory(rng):
    case = base(rng, 'inventory', 'inventory')
    scale = rng.choice([1.0, 1.0, 1.0, 0.5, 2.0, 0.0625, 15.0])
    height = rng.choice([1.8, 1.8, 1.5, 0.6])
    case['figure'] = {'bbWidth': 0.6 * scale, 'bbHeight': height * scale, 'scale': scale, 'fallFlying': rng.random() < 0.15}
    return case


def search(rng):
    case = base(rng, 'search', 'search')
    text = rng.choice(QUERIES)
    cursor = rng.choice([len(text), len(text), 0, rng.randint(0, len(text))])
    highlight = rng.choice([cursor, cursor, 0, len(text), rng.randint(0, len(text))])
    case['search'] = {'text': text, 'cursor': cursor, 'highlight': highlight, 'blink': rng.randint(0, 1)}
    return case


def hotbar(rng):
    case = base(rng, 'hotbar', 'hotbar')
    rows = []
    for _ in range(9):
        row = [stack(rng) if rng.random() < 0.4 else None for _ in range(9)]
        row[rng.randint(0, 8)] = stack(rng)
        rows.append(row)
    case['hotbars'] = rows
    return case


GENERATORS = {'category': category, 'inventory': inventory, 'search': search, 'hotbar': hotbar}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--count', type=int, default=2400)
    parser.add_argument('--kind', action='append', choices=sorted(GENERATORS))
    parser.add_argument('--seed', type=int, default=0x63726561)
    args = parser.parse_args()
    kinds = args.kind or list(GENERATORS)
    rng = random.Random(args.seed)
    cases = [GENERATORS[kinds[n % len(kinds)]](rng) for n in range(args.count)]
    cases.sort(key=lambda c: c['op'] and c['opTab'])
    for case in cases:
        print(json.dumps(case, separators=(',', ':')))


if __name__ == '__main__':
    main()
