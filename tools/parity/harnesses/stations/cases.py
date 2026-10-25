#!/usr/bin/env python3
"""Random station screen cases (anvil, grindstone, smithing, cartography, enchanting, beacon), one
generator per kind, drawn round-robin; one JSON case per line.

cases.py [--count N] [--kind K]... [--seed S]

A case: kind, GUI size, mouse, partial tick, title (component JSON or null), the menu's slots
([index, stack], set in index order; results the client computes itself are left out), the
carried stack, data slots ([index, value]) and the kind's own state. Stacks are ItemStack.CODEC
JSON. The mouse avoids slots whose stacks have more to their tooltip than a name (the tooltips
harness's area).
"""
import argparse
import json
import random

SIZES = [(427, 240), (320, 240), (480, 270), (640, 360), (961, 501), (230, 219), (250, 230), (379, 250), (378, 250)]
PLAIN = [('minecraft:stone', 64), ('minecraft:dirt', 64), ('minecraft:apple', 64), ('minecraft:torch', 64),
         ('minecraft:ender_pearl', 16), ('minecraft:bucket', 16), ('minecraft:cake', 1), ('minecraft:bread', 64),
         ('minecraft:iron_ingot', 64), ('minecraft:diamond', 64), ('minecraft:paper', 64), ('minecraft:glass_pane', 64),
         ('minecraft:lapis_lazuli', 64), ('minecraft:emerald', 64), ('minecraft:map', 64), ('minecraft:netherite_ingot', 64)]
TITLES = [None, None, None, None, {'text': 'Forge'}, {'text': 'A long custom station title that overflows'},
          {'text': 'Red', 'color': 'red'}, {'text': ''}]
GEAR = ['minecraft:diamond_sword', 'minecraft:iron_pickaxe', 'minecraft:golden_helmet', 'minecraft:diamond_chestplate',
        'minecraft:iron_leggings', 'minecraft:leather_boots', 'minecraft:bow', 'minecraft:trident', 'minecraft:shears',
        'minecraft:netherite_axe', 'minecraft:fishing_rod', 'minecraft:elytra']
ENCHANTMENTS = [('sharpness', 5), ('protection', 4), ('unbreaking', 3), ('efficiency', 5), ('mending', 1),
                ('fire_aspect', 2), ('binding_curse', 1), ('vanishing_curse', 1), ('silk_touch', 1), ('looting', 3),
                ('power', 5), ('infinity', 1), ('feather_falling', 4), ('thorns', 3), ('respiration', 3)]
NAMES = ['', 'Excalibur', 'a', 'Pick of Destiny', 'x' * 35, 'The quick brown fox jumps over the lazy dog!', 'iiiiiiiiii', 'WWWWWWWWWW']
EFFECTS = ['speed', 'haste', 'resistance', 'jump_boost', 'strength', 'regeneration']
TEMPLATES = ['minecraft:netherite_upgrade_smithing_template', 'minecraft:coast_armor_trim_smithing_template',
             'minecraft:sentry_armor_trim_smithing_template', 'minecraft:silence_armor_trim_smithing_template']
INVENTORY = [(8 + c * 18, 84 + r * 18) for r in range(3) for c in range(9)] + [(8 + c * 18, 142) for c in range(9)]

# kind: (image size, station slots [(x, y)], index of a result the client computes or None)
KINDS = {
    'anvil': ((176, 166), [(27, 47), (76, 47), (134, 47)], 2),
    'grindstone': ((176, 166), [(49, 19), (49, 40), (129, 34)], 2),
    'smithing': ((176, 166), [(8, 48), (26, 48), (44, 48), (98, 48)], None),
    'cartography': ((176, 166), [(15, 15), (15, 52), (145, 39)], None),
    'enchanting': ((176, 166), [(15, 47), (35, 47)], None),
    'beacon': ((230, 219), [(136, 110)], None),
}


def plain(rng, count=None):
    item, most = rng.choice(PLAIN)
    return {'id': item, 'count': count or rng.choice([1, 1, most, rng.randint(1, most)])}


def enchantments(rng, n):
    return {'minecraft:' + e: rng.randint(1, top) for e, top in rng.sample(ENCHANTMENTS, n)}


def gear(rng):
    s = {'id': rng.choice(GEAR), 'count': 1, 'components': {}}
    if rng.random() < 0.5:
        s['components']['minecraft:damage'] = rng.randint(0, 200)
    if rng.random() < 0.5:
        s['components']['minecraft:enchantments'] = enchantments(rng, rng.randint(1, 3))
    if rng.random() < 0.3:
        s['components']['minecraft:repair_cost'] = rng.choice([0, 1, 3, 7, 15, 31, 39, 63])
    if rng.random() < 0.2:
        s['components']['minecraft:custom_name'] = rng.choice(['Old Blade', {'text': 'Fancy', 'color': 'gold'}])
    return s


def book(rng):
    return {'id': 'minecraft:enchanted_book', 'count': 1, 'components': {'minecraft:stored_enchantments': enchantments(rng, rng.randint(1, 2))}}


def is_plain(s):
    return s is not None and 'components' not in s and any(s['id'] == p for p, _ in PLAIN)


def station_slots(rng, kind):
    """{slot index: stack} for the station's own slots."""
    if kind in ('anvil', 'grindstone'):
        pick = lambda: rng.choice([None, gear(rng), gear(rng), book(rng), plain(rng), plain(rng, 1)])
        return {i: s for i, s in ((0, pick()), (1, pick())) if s}
    if kind == 'smithing':
        out = {}
        if rng.random() < 0.7:
            out[0] = {'id': rng.choice(TEMPLATES), 'count': rng.choice([1, 1, 5])}
        if rng.random() < 0.6:
            out[1] = gear(rng)
        if rng.random() < 0.5:
            out[2] = {'id': rng.choice(['minecraft:netherite_ingot', 'minecraft:diamond', 'minecraft:amethyst_shard', 'minecraft:stone']), 'count': rng.randint(1, 64)}
        if rng.random() < 0.4 and out:
            out[3] = gear(rng)
        return out
    if kind == 'cartography':
        out = {}
        if rng.random() < 0.8:
            out[0] = rng.choice([{'id': 'minecraft:filled_map', 'count': 1, 'components': {'minecraft:map_id': rng.randint(0, 3)}},
                                 {'id': 'minecraft:filled_map', 'count': 1}, plain(rng)])
        if rng.random() < 0.8:
            out[1] = {'id': rng.choice(['minecraft:map', 'minecraft:paper', 'minecraft:glass_pane', 'minecraft:stone']), 'count': rng.randint(1, 64)}
        if rng.random() < 0.5:
            out[2] = {'id': 'minecraft:filled_map', 'count': rng.randint(1, 2), 'components': {'minecraft:map_id': rng.randint(0, 3)}}
        return out
    if kind == 'enchanting':
        out = {}
        if rng.random() < 0.8:
            out[0] = rng.choice([gear(rng), {'id': 'minecraft:book', 'count': 1}, plain(rng, 1)])
        if rng.random() < 0.7:
            out[1] = {'id': 'minecraft:lapis_lazuli', 'count': rng.choice([1, 2, 3, 64, rng.randint(1, 64)])}
        return out
    return {0: rng.choice([{'id': 'minecraft:iron_ingot', 'count': 1}, {'id': 'minecraft:netherite_ingot', 'count': 1}])} if rng.random() < 0.5 else {}


def mouse(rng, width, height, left, top, image, avoid):
    for _ in range(20):
        if rng.random() < 0.85:
            m = [rng.randint(left - 4, left + image[0] + 4), rng.randint(top - 4, top + image[1] + 4)]
        else:
            m = [rng.randint(0, width), rng.randint(0, height)]
        x, y = m[0] - left, m[1] - top
        if not any(sx - 1 <= x < sx + 17 and sy - 1 <= y < sy + 17 for sx, sy in avoid):
            return m
    return [0, 0]


def case(rng, kind):
    image, positions, computed = KINDS[kind]
    width, height = rng.choice(SIZES)
    left, top = (width - image[0]) // 2, (height - image[1]) // 2
    own = station_slots(rng, kind)
    inventory_at = (36, 137) if kind == 'beacon' else (8, 84)
    first = len(positions)
    slots = dict(own)
    for i in range(36):
        if rng.random() < rng.choice([0.0, 0.2, 0.5]):
            slots[first + i] = rng.choice([plain(rng), plain(rng), gear(rng)])
    every = positions + [(inventory_at[0] + x - 8, inventory_at[1] + y - 84) for x, y in INVENTORY]
    avoid = [every[i] for i, s in slots.items() if not is_plain(s)]
    if computed is not None and own:
        avoid.append(positions[computed])
    c = {'kind': kind, 'width': width, 'height': height, 'mouse': mouse(rng, width, height, left, top, image, avoid),
         'partial': round(rng.random(), 3), 'title': rng.choice(TITLES), 'advanced': rng.random() < 0.2,
         'slots': [[i, s] for i, s in sorted(slots.items())], 'xp': rng.choice([0, 1, 5, 30, 39, 40, 100]),
         'creative': rng.random() < 0.15}
    if rng.random() < 0.2:
        c['carried'] = plain(rng)
    return c


def anvil(rng):
    c = case(rng, 'anvil')
    if rng.random() < 0.4:
        c['name'] = rng.choice(NAMES)
    c['blink'] = rng.random() < 0.3
    if rng.random() < 0.3:
        c['data'] = [[0, rng.choice([0, 1, 5, 39, 40, 41, 100])]]
    return c


def smithing(rng):
    c = case(rng, 'smithing')
    c['ticks'] = rng.choice([0, 1, 29, 30, 31, 32, 33, 34, 60, 61, 95, rng.randint(0, 400)])
    c['data'] = [[0, int(rng.random() < 0.4)]]
    return c


def cartography(rng):
    c = case(rng, 'cartography')
    c['maps'] = [[i, rng.randint(0, 4), rng.random() < 0.3] for i in range(4) if rng.random() < 0.8]
    return c


def enchanting(rng):
    c = case(rng, 'enchanting')
    costs = [rng.choice([0, 0, rng.randint(1, 8), rng.randint(1, 30), 30]) for _ in range(3)]
    c['data'] = [[i, v] for i, v in enumerate(costs)] + [[3, rng.randint(-2**31, 2**31 - 1)]]
    c['clues'] = [[i, 'minecraft:' + e, rng.choice([-1, 1, 1, 2, top, rng.randint(1, 10)])]
                  for i, (e, top) in enumerate(rng.choice(ENCHANTMENTS) for _ in range(3)) if rng.random() < 0.8]
    c['open'] = [round(rng.choice([0, 1, rng.random()]), 3), round(rng.choice([0, 1, rng.random()]), 3)]
    c['flip'] = [round(rng.uniform(-3, 3), 3), round(rng.uniform(-3, 3), 3)]
    if rng.random() < 0.5:
        i = rng.randrange(3)
        c['mouse'] = [(c['width'] - 176) // 2 + 60 + rng.randint(-2, 109), (c['height'] - 166) // 2 + 14 + 19 * i + rng.randint(-2, 19)]
    return c


def beacon(rng):
    c = case(rng, 'beacon')
    c['levels'] = rng.choice([0, 1, 2, 3, 4, 4, 4])
    c['primary'] = rng.choice([None, None] + ['minecraft:' + e for e in EFFECTS[:5]])
    c['secondary'] = rng.choice([None, None, 'minecraft:regeneration', c['primary']])
    if rng.random() < 0.6:
        left, top = (c['width'] - 230) // 2, (c['height'] - 219) // 2
        c['mouse'] = [left + rng.randint(10, 215), top + rng.randint(18, 132)]
    return c


GENERATORS = {'anvil': anvil, 'grindstone': lambda rng: case(rng, 'grindstone'), 'smithing': smithing,
              'cartography': cartography, 'enchanting': enchanting, 'beacon': beacon}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--count', type=int, default=2400)
    parser.add_argument('--kind', action='append', choices=sorted(GENERATORS))
    parser.add_argument('--seed', type=int, default=0x73746174)
    args = parser.parse_args()
    kinds = args.kind or list(GENERATORS)
    rng = random.Random(args.seed)
    for n in range(args.count):
        print(json.dumps(GENERATORS[kinds[n % len(kinds)]](rng), separators=(',', ':')))


if __name__ == '__main__':
    main()
