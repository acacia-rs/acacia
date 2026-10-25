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


EFFECTS = ['speed', 'slowness', 'haste', 'mining_fatigue', 'strength', 'jump_boost', 'nausea', 'regeneration', 'resistance',
           'fire_resistance', 'water_breathing', 'invisibility', 'blindness', 'night_vision', 'hunger', 'weakness', 'poison',
           'wither', 'health_boost', 'absorption', 'glowing', 'levitation', 'luck', 'unluck', 'slow_falling', 'conduit_power',
           'dolphins_grace', 'bad_omen', 'hero_of_the_village', 'darkness', 'trial_omen', 'raid_omen', 'wind_charged',
           'weaving', 'oozing', 'infested']


def effects(rng):
    """EffectsInInventory beside a chest window: [id, duration, amplifier, ambient] per effect."""
    rows = rng.randint(1, 6)
    width, height = rng.choice([(427, 240), (480, 270), (640, 360), (961, 501), (379, 250), (400, 240), (360, 240), (900, 300)])
    left, top = (width - 176) // 2, (height - 114 - rows * 18) // 2
    durations = [-1, 0, 1, 19, 20, 600, 1199, 3600, 32147, 32148, 72000, 1728000, rng.randint(0, 100000)]
    chosen = rng.sample(EFFECTS, rng.choice([0, 1, 1, 2, 3, 5, 6, 8, 12]))
    case = {'kind': 'effects', 'rows': rows, 'width': width, 'height': height, 'title': None,
            'effects': [['minecraft:' + e, rng.choice(durations), rng.choice([0, 0, 1, 2, 4, 8, 9, 10, 255]), rng.random() < 0.2]
                        for e in chosen]}
    if rng.random() < 0.7:
        case['mouse'] = [rng.randint(left + 170, width), rng.randint(top - 5, top + 170)]
    else:
        case['mouse'] = [rng.randint(0, width), rng.randint(0, height)]
    return case


MOUNTS = ['horse', 'donkey', 'mule', 'skeleton_horse', 'zombie_horse', 'llama', 'trader_llama', 'camel', 'camel_husk',
          'nautilus', 'zombie_nautilus']


def mount(rng):
    """A mount's inventory: entity, tamed/baby/chest/name, chest columns and last frame's mouse."""
    entity = rng.choice(MOUNTS)
    columns = 0
    chest = rng.random() < 0.5
    if chest and entity in ('donkey', 'mule'):
        columns = 5
    elif chest and entity.endswith('llama'):
        columns = 3 * rng.randint(1, 5)
    case = base(rng, 'mount', (176, 166), 2 + 3 * columns + 36)
    case.update({'entity': 'minecraft:' + entity, 'columns': columns, 'chest': chest, 'tamed': rng.random() < 0.8,
                 'baby': rng.random() < 0.15, 'title': None})
    if rng.random() < 0.3:
        case['name'] = rng.choice(['Spirit', 'A horse with a very long name indeed', 'x'])
    if 'quick' in case:
        case['quick']['slots'] = [s for s in case['quick']['slots'] if s >= 2] or [2]
    w, h = case['width'], case['height']
    case['lastMouse'] = rng.choice([case['mouse'], [rng.randint(0, w), rng.randint(0, h)], [0, 0]])
    return case


WORDS = ['the', 'quick', 'brown', 'fox', 'jumps', 'over', 'lazy', 'dog', 'Minecraft', 'lectern', 'a', 'I',
         'supercalifragilisticexpialidocious', 'WWWWWWWWWWWWWWWWWWWWWWWWWW', 'iiiiii', 'page', '42', '...', 'end.']


def page_text(rng):
    words = [rng.choice(WORDS) for _ in range(rng.choice([0, 1, 5, 20, 60, 140]))]
    text = ''
    for w in words:
        text += w + rng.choice([' ', ' ', ' ', '\n', '  ', '\n\n'])
    return text


def page_component(rng):
    text = page_text(rng)
    if rng.random() < 0.4:
        return text
    parts = [{'text': page_text(rng)[:rng.randint(0, 60)], 'color': rng.choice(['red', 'dark_blue', '#00ff88', 'black'])}]
    if rng.random() < 0.5:
        parts.append({'text': ' bold words ', 'bold': True})
    return {'text': text[:rng.randint(0, len(text))], 'extra': parts}


def lectern(rng):
    """LecternScreen: a written or writable book (or none) open at a page; may build or not."""
    width, height = rng.choice(SIZES + [(192, 230), (220, 216)])
    count = rng.choice([0, 1, 2, 3, 7, 30])
    book = rng.choice(['written', 'written', 'writable', 'plain', None])
    case = {'kind': 'lectern', 'width': width, 'height': height, 'page': rng.choice([0, 0, 1, 2, 5, 29, 40, -1]),
            'build': rng.random() < 0.7, 'partial': 0.0}
    if book == 'written':
        case['book'] = {'id': 'minecraft:written_book', 'components': {'minecraft:written_book_content': {
            'title': 'A Book', 'author': 'Someone', 'resolved': True, 'pages': [page_component(rng) for _ in range(count)]}}}
    elif book == 'writable':
        case['book'] = {'id': 'minecraft:writable_book', 'components': {'minecraft:writable_book_content': {
            'pages': [page_text(rng)[:1024] for _ in range(count)]}}}
    elif book == 'plain':
        case['book'] = {'id': 'minecraft:book'}
    left = (width - 192) // 2
    if rng.random() < 0.6:
        case['mouse'] = [rng.randint(left + 30, left + 150), rng.randint(150, 220)]
    else:
        case['mouse'] = [rng.randint(0, width), rng.randint(0, height)]
    return case


GENERATORS = {k: make(k) for k in KINDS}
GENERATORS['lectern'] = lectern
GENERATORS['effects'] = effects
GENERATORS['mount'] = mount


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--count', type=int, default=2700)
    parser.add_argument('--kind', action='append', choices=sorted(GENERATORS))
    parser.add_argument('--seed', type=int, default=0x636f6e74)
    args = parser.parse_args()
    kinds = args.kind or list(GENERATORS)
    rng = random.Random(args.seed)
    for n in range(args.count):
        print(json.dumps(GENERATORS[kinds[n % len(kinds)]](rng), separators=(',', ':')))


if __name__ == '__main__':
    main()
