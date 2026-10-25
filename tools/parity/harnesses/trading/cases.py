#!/usr/bin/env python3
"""Random list-screen cases (merchant, stonecutter, loom), drawn round-robin; one JSON case per line.

cases.py [--count N] [--kind K]...

A case: kind, GUI size, mouse, partial tick, title (component JSON or null for the default),
inventory slot contents ([index, stack]), the carried stack, plus the kind's own state: offers and
villager level/xp (merchant); the input, a clicked recipe and wheel steps (stonecutter); banner, dye
and pattern stacks, a clicked pattern and wheel steps (loom). Stacks are ItemStack.CODEC JSON.
"""
import argparse
import json
import random

SIZES = [(427, 240), (320, 240), (480, 270), (640, 360), (961, 501), (276, 166), (300, 200), (379, 250), (378, 250)]
# (id, max stack) of items whose tooltips are a name and little else.
PLAIN = [('minecraft:emerald', 64), ('minecraft:wheat', 64), ('minecraft:bread', 64), ('minecraft:apple', 64),
         ('minecraft:coal', 64), ('minecraft:paper', 64), ('minecraft:iron_ingot', 64), ('minecraft:stick', 64),
         ('minecraft:ender_pearl', 16), ('minecraft:bucket', 16), ('minecraft:saddle', 1), ('minecraft:cake', 1),
         ('minecraft:glass', 64), ('minecraft:white_wool', 64), ('minecraft:arrow', 64), ('minecraft:book', 64),
         ('minecraft:nether_star', 64), ('minecraft:golden_apple', 64), ('minecraft:compass', 64)]
STONES = ['minecraft:stone', 'minecraft:cobblestone', 'minecraft:stone_bricks', 'minecraft:andesite', 'minecraft:granite',
          'minecraft:diorite', 'minecraft:sandstone', 'minecraft:red_sandstone', 'minecraft:quartz_block',
          'minecraft:copper_block', 'minecraft:cut_copper', 'minecraft:deepslate', 'minecraft:cobbled_deepslate',
          'minecraft:tuff', 'minecraft:blackstone', 'minecraft:bricks', 'minecraft:prismarine', 'minecraft:purpur_block',
          'minecraft:end_stone', 'minecraft:mud_bricks', 'minecraft:polished_andesite', 'minecraft:dirt']
COLORS = ['white', 'orange', 'magenta', 'light_blue', 'yellow', 'lime', 'pink', 'gray', 'light_gray', 'cyan',
          'purple', 'blue', 'brown', 'green', 'red', 'black']
PATTERN_ITEMS = [None, None, None, 'minecraft:flower_banner_pattern', 'minecraft:creeper_banner_pattern',
                 'minecraft:skull_banner_pattern', 'minecraft:mojang_banner_pattern', 'minecraft:globe_banner_pattern',
                 'minecraft:piglin_banner_pattern', 'minecraft:flow_banner_pattern', 'minecraft:guster_banner_pattern',
                 'minecraft:field_masoned_banner_pattern', 'minecraft:bordure_indented_banner_pattern']
LAYERS = ['minecraft:stripe_bottom', 'minecraft:cross', 'minecraft:border', 'minecraft:gradient', 'minecraft:creeper',
          'minecraft:circle', 'minecraft:rhombus', 'minecraft:half_vertical']
TITLES = [None, None, None, {'text': 'Farmer'}, {'translate': 'entity.minecraft.villager.librarian'},
          {'text': 'A very long custom trader name that overflows'}, {'text': 'Red', 'color': 'red'}, {'text': ''}]
# kind: (image width, image height, menu slots before the inventory)
KINDS = {'merchant': (276, 166, 3), 'stonecutter': (176, 166, 2), 'loom': (176, 166, 4)}
# Slots whose stacks have tooltip lines acacia does not build yet (banner patterns): never hovered.
LOOM_DETAIL = [(13, 26), (23, 45), (143, 57)]
# Window boxes (x0, y0, x1, y1) the mouse is often put in: lists, scrollers, the out-of-stock mark.
LISTS = {'merchant': [(3, 16, 101, 160), (182, 32, 210, 58)], 'stonecutter': [(50, 14, 134, 72)],
         'loom': [(58, 11, 134, 72)]}


def stack(rng, sizes=True):
    item, most = rng.choice(PLAIN)
    count = rng.choice([1, 1, most, rng.randint(1, most)]) if sizes else 1
    s = {'id': item, 'count': count}
    if rng.random() < 0.1:
        s['components'] = {'minecraft:custom_name': rng.choice(['Named', {'text': 'Fancy', 'color': 'gold'}])}
    return s


def cost(rng):
    item, most = rng.choice(PLAIN)
    return {'id': item, 'count': rng.choice([1, 1, rng.randint(1, most), most])}


def offer(rng):
    o = {'buy': cost(rng), 'sell': stack(rng), 'maxUses': rng.choice([1, 4, 12, 16]), 'xp': rng.randint(0, 30)}
    o['uses'] = rng.choice([0, 0, rng.randint(0, o['maxUses']), o['maxUses']])
    if rng.random() < 0.5:
        o['buyB'] = cost(rng)
    if rng.random() < 0.4:
        o['specialPrice'] = rng.randint(-20, 5)
    if rng.random() < 0.4:
        o['demand'] = rng.randint(-5, 20)
        o['priceMultiplier'] = rng.choice([0.05, 0.2, 0.0])
    return o


def mouse(rng, width, height, image, avoid=(), lists=()):
    left, top = (width - image[0]) // 2, (height - image[1]) // 2
    while True:
        roll = rng.random()
        if roll < 0.45 and lists:
            x0, y0, x1, y1 = rng.choice(lists)
            m = [left + rng.randint(x0, x1), top + rng.randint(y0, y1)]
        elif roll < 0.9:
            m = [rng.randint(left - 4, left + image[0] + 4), rng.randint(top - 4, top + image[1] + 4)]
        else:
            m = [rng.randint(0, width), rng.randint(0, height)]
        if not any(left + x - 1 <= m[0] < left + x + 17 and top + y - 1 <= m[1] < top + y + 17 for x, y in avoid):
            return m


def base(rng, kind):
    image_w, image_h, menu_slots = KINDS[kind]
    width, height = rng.choice(SIZES)
    case = {'kind': kind, 'width': width, 'height': height, 'partial': round(rng.random(), 3),
            'mouse': mouse(rng, width, height, (image_w, image_h), LOOM_DETAIL if kind == 'loom' else (), LISTS[kind]),
            'title': rng.choice(TITLES), 'advanced': rng.random() < 0.2,
            'slots': [[i, stack(rng)] for i in range(menu_slots, menu_slots + 36) if rng.random() < rng.choice([0.0, 0.3, 0.6])]}
    if rng.random() < 0.15:
        case['carried'] = stack(rng)
    return case


def merchant(rng):
    case = base(rng, 'merchant')
    n = rng.choice([0, 1, 3, 7, 8, rng.randint(1, 20)])
    case['offers'] = [offer(rng) for _ in range(n)]
    case['level'] = rng.choice([0, 1, 2, 3, 4, 5, 6])
    case['xp'] = rng.choice([0, 5, 10, 69, 70, 100, 149, 200, 260, rng.randint(0, 300)])
    case['bar'] = rng.random() < 0.7
    case['restock'] = rng.random() < 0.6
    case['shop'] = rng.randrange(n) if n else 0
    case['scroll'] = rng.randint(0, max(0, n - 7))
    for i in (0, 1):
        if rng.random() < 0.3:
            case['slots'].append([i, cost(rng)])
    return case


def wheel(rng):
    return [rng.choice([-1.0, -1.0, 1.0, -3.0, -0.5, 2.0]) for _ in range(rng.choice([0, 0, 1, 2, 4]))]


def stonecutter(rng):
    case = base(rng, 'stonecutter')
    if rng.random() < 0.9:
        case['input'] = {'id': rng.choice(STONES), 'count': rng.choice([1, 1, 64, rng.randint(1, 64)])}
    if rng.random() < 0.6:
        case['select'] = rng.choice([0, 1, 2, rng.randint(0, 30)])
    case['scrolls'] = wheel(rng)
    return case


def loom(rng):
    case = base(rng, 'loom')
    if rng.random() < 0.85:
        banner = {'id': f'minecraft:{rng.choice(COLORS)}_banner', 'count': rng.choice([1, 1, 16])}
        if rng.random() < 0.35:
            layers = rng.choice([1, 2, 5, 6, 6, 7])
            banner['components'] = {'minecraft:banner_patterns': [{'pattern': rng.choice(LAYERS), 'color': rng.choice(COLORS)} for _ in range(layers)]}
        case['banner'] = banner
    if rng.random() < 0.85:
        case['dye'] = {'id': f'minecraft:{rng.choice(COLORS)}_dye', 'count': rng.choice([1, 64])}
    pattern = rng.choice(PATTERN_ITEMS)
    if pattern:
        case['pattern'] = {'id': pattern, 'count': 1}
    if rng.random() < 0.6:
        case['select'] = rng.choice([0, 1, 5, rng.randint(0, 40)])
    case['scrolls'] = wheel(rng)
    return case


GENERATORS = {'merchant': merchant, 'stonecutter': stonecutter, 'loom': loom}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--count', type=int, default=2400)
    parser.add_argument('--kind', action='append', choices=sorted(GENERATORS))
    parser.add_argument('--seed', type=int, default=0x74726164)
    args = parser.parse_args()
    kinds = args.kind or list(GENERATORS)
    rng = random.Random(args.seed)
    for n in range(args.count):
        print(json.dumps(GENERATORS[kinds[n % len(kinds)]](rng), separators=(',', ':')))


if __name__ == '__main__':
    main()
