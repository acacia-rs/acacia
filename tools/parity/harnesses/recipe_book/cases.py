#!/usr/bin/env python3
"""Random recipe book cases, kinds drawn round-robin; one JSON case per line.

cases.py [--count N] [--kind K]... [--seed S]

A case: kind (a screen, or `book_<screen>` for the component alone), GUI size, mouse, partial
tick, the book's state (open, filtering, `recipes` sampled from the book type's recipes with
`seed`, selected tab, page, time, tab and button animation times, the search box, a page button's
overlay, a ghost recipe), the menu's slots ([index, stack]; the inventory's make recipes
craftable), a carried stack, a furnace's data, and for the inventory the player's figure, the
last frame's mouse and active effects. Stacks are ItemStack.CODEC JSON.
"""
import argparse
import json
import random

SIZES = [(427, 240), (480, 270), (640, 360), (320, 240), (378, 260), (379, 260), (854, 480), (400, 300), (961, 501)]
INGREDIENTS = ['oak_planks', 'stick', 'cobblestone', 'iron_ingot', 'coal', 'oak_log', 'raw_iron', 'gold_ingot', 'string',
               'redstone', 'wheat', 'sand', 'beef', 'potato', 'diamond', 'white_wool', 'glass', 'stone', 'copper_ingot',
               'kelp', 'raw_copper', 'cod', 'chicken', 'cactus', 'clay_ball', 'leather', 'paper', 'feather', 'flint',
               'sugar_cane', 'egg', 'birch_planks', 'cobbled_deepslate', 'quartz', 'iron_nugget', 'bamboo', 'slime_ball']
WORDS = ['', '', 'a', 'iron', 'oak planks', 'stone bricks', 'cake', 'zzzzzzzzzzzzzzzzzzzzzzzzzzzzz', 'Furnace', 'w']
EFFECTS = ['speed', 'haste', 'strength', 'regeneration', 'fire_resistance', 'night_vision', 'poison', 'luck', 'slow_falling']
SCREENS = {
    # kind: (inventory slots, grid slots, furnace)
    'crafting': (range(10, 46), range(1, 10), False),
    'furnace': (range(3, 39), [0, 1], True),
    'blast_furnace': (range(3, 39), [0, 1], True),
    'smoker': (range(3, 39), [0, 1], True),
    'inventory': (range(9, 45), range(1, 5), False),
}
TABS = {'crafting': 5, 'inventory': 5, 'furnace': 4, 'blast_furnace': 3, 'smoker': 2}


def stack(rng):
    item = rng.choice(INGREDIENTS)
    return {'id': 'minecraft:' + item, 'count': rng.choice([1, 1, 4, 16, 64, rng.randint(1, 64)])}


def anim(rng):
    return rng.choice([0.0, 0.0, 0.0, round(rng.uniform(0.1, 15), 3), 15.0])


def geometry(case):
    """The book's origin and the window's corner, as RecipeBookComponent and updateScreenPosition place them."""
    width, height, open_ = case['width'], case['height'], case['open']
    narrow = width < 379
    xo, yo = (width - 147) // 2 - (0 if narrow else 86), (height - 166) // 2
    left = 177 + (width - 176 - 200) // 2 if open_ and not narrow else (width - 176) // 2
    return xo, yo, left, (height - 166) // 2


def targets(rng, case, screen):
    xo, yo, left, top = geometry(case)
    out = [(xo + 11 + 25 * (i % 5) + rng.randint(0, 24), yo + 31 + 25 * (i // 5) + rng.randint(0, 24)) for i in range(20)]
    out += [(xo + 110 + rng.randint(0, 25), yo + 12 + rng.randint(0, 15)), (xo + 93 + rng.randint(0, 11), yo + 140),
            (xo + 38 + rng.randint(0, 11), yo + 140), (xo + 30, yo + 18), (xo - 20, yo + 5 + 27 * rng.randint(0, 4)),
            (xo + 40 + rng.randint(0, 80), yo + 40 + rng.randint(0, 100))]
    if screen:
        out += [(left + rng.randint(0, 175), top + rng.randint(0, 165)) for _ in range(8)]
        out += [(left + rng.choice([5, 20, 104]) + rng.randint(0, 19), case['height'] // 2 - rng.choice([49, 22]) + rng.randint(0, 17))]
    out.append((rng.randint(0, case['width']), rng.randint(0, case['height'])))
    return out


def make(kind):
    bare = kind.startswith('book_')
    screen = kind[5:] if bare else kind

    def gen(rng):
        width, height = rng.choice(SIZES)
        value = rng.choice(WORDS)
        cursor = rng.randint(0, len(value))
        case = {'kind': kind, 'width': width, 'height': height, 'partial': round(rng.random(), 3),
                'open': bare or rng.random() < 0.75, 'filtering': rng.random() < 0.3,
                'recipes': rng.choice([0, 1, 5, 19, 20, 21, 47, 80, 200, 400]), 'seed': rng.randint(0, 1 << 30),
                'tab': rng.randrange(TABS[screen]), 'page': rng.choice([0, 0, 1, 2, 5]),
                'time': rng.choice([0.0, 0.0, 29.5, 30.0, 59.0, 61.0, 150.0, round(rng.uniform(0, 3000), 2)]),
                'tabAnims': [anim(rng) for _ in range(TABS[screen])], 'buttonAnims': [anim(rng) for _ in range(20)],
                'search': {'value': value, 'cursor': cursor, 'highlight': rng.choice([cursor, rng.randint(0, len(value))]),
                           'display': rng.choice([0, 0, min(cursor, rng.randint(0, len(value)))]),
                           'focused': rng.random() < 0.4, 'blink': rng.choice([0, 150, 450, 700])}}
        inventory, grid, furnace = SCREENS[screen]
        case['slots'] = [[i, stack(rng)] for i in inventory if rng.random() < rng.choice([0.0, 0.2, 0.5, 0.9])]
        case['slots'] += [[i, stack(rng)] for i in grid if rng.random() < 0.2]
        if rng.random() < 0.35:
            case['overlay'] = rng.randrange(rng.choice([1, 2, 4, 8, 20]))
        if rng.random() < 0.35:
            case['ghost'] = rng.randrange(1000)
        if not bare and rng.random() < 0.15:
            case['carried'] = stack(rng)
        if furnace:
            total = rng.choice([200, 100, 0])
            case['data'] = [rng.choice([0, 0, 1, 100, 200, 300]), rng.choice([0, 200, 1600]), rng.randint(0, max(total, 1)), total]
        if screen == 'inventory':
            case['figure'] = {'bbHeight': rng.choice([1.8, 1.8, 1.5, 0.6]), 'scale': rng.choice([1.0, 1.0, 0.5, 2.0]),
                              'fallFlying': rng.random() < 0.1}
            case['last'] = [round(rng.uniform(0, width), 2), round(rng.uniform(0, height), 2)]
            case['effects'] = [['minecraft:' + e, rng.choice([-1, 200, 3600, 100000]), rng.choice([0, 1]), rng.random() < 0.2]
                               for e in rng.sample(EFFECTS, rng.choice([0, 0, 0, 1, 3]))]
        case['mouse'] = list(rng.choice(targets(rng, case, not bare)))
        return case
    return gen


GENERATORS = {k: make(k) for k in [*SCREENS, *('book_' + s for s in SCREENS)]}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--count', type=int, default=2500)
    parser.add_argument('--kind', action='append', choices=sorted(GENERATORS))
    parser.add_argument('--seed', type=int, default=0x7265_6370)
    args = parser.parse_args()
    kinds = args.kind or list(GENERATORS)
    rng = random.Random(args.seed)
    for n in range(args.count):
        print(json.dumps(GENERATORS[kinds[n % len(kinds)]](rng), separators=(',', ':')))


if __name__ == '__main__':
    main()
