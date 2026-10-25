#!/usr/bin/env python3
"""Player lists: listed players (game mode, latency, display name, list order, hat layer, in the
level for upside-down Dinnerbone and Grumm), teams, a `list` objective (integer or hearts, number
formats), header, footer, row background option and frames of score changes and Hud ticks. One
JSON case per frame: the list up to and including that frame, whose draws are compared.

cases.py [--count N]   N lists (Horizon's test_tab_list_parity.py, MIT, (c) Ludovic Marie)
"""
import argparse
import json
import random
import uuid

COLORS = ['black', 'dark_blue', 'dark_green', 'dark_aqua', 'dark_red', 'dark_purple', 'gold', 'gray', 'dark_gray',
          'blue', 'green', 'aqua', 'red', 'light_purple', 'yellow', 'white']
WORDS = ['Welcome', 'to', 'the', 'server', 'Acacia', 'players', 'online', 'mc.example.net', 'Have', 'fun!',
         'Supercalifragilisticexpialidocious', 'a', 'é', 'Ünïcödé', '§aGreen', '§lBold', '[VIP]', '1234567890']
NAMES = ['Steve', 'Alex', 'Notch', 'jeb_', 'Dinnerbone', 'Grumm', 'xX_Pro_Xx', 'a', 'Zed', 'zebra', 'Émile', 'ÆON',
         'bob', 'Carl', '_under', '123', 'MMMMMMMMMMMMMMMM', 'iiiiiiii', 'Wide_W', 'ßtraße', 'Ⅻ', 'Mix3d_Case']


def component(rng, words=3, depth=0):
    text = ' '.join(rng.choice(WORDS) for _ in range(words))
    if rng.random() < 0.3:
        return text
    c = {'text': text}
    if rng.random() < 0.5:
        c['color'] = rng.choice(COLORS + ['#12ABEF'])
    for flag in ('bold', 'italic', 'underlined', 'strikethrough', 'obfuscated'):
        if rng.random() < 0.15:
            c[flag] = rng.random() < 0.8
    if depth < 2 and rng.random() < 0.4:
        c['extra'] = [component(rng, rng.randint(1, 3), depth + 1) for _ in range(rng.randint(1, 2))]
    if rng.random() < 0.08:
        c = {'translate': 'multiplayer.player.list.hp', 'with': [text], 'color': rng.choice(COLORS)}
    return c


def number_format(rng):
    kind = rng.random()
    if kind < 0.4:
        return None
    if kind < 0.55:
        return {'type': 'blank'}
    if kind < 0.85:
        style = {'color': rng.choice(COLORS)}
        if rng.random() < 0.4:
            style['bold'] = True
        return {'type': 'styled', 'style': style}
    return {'type': 'fixed', 'value': component(rng, 1)}


def players_of(rng):
    size = rng.choice([1, 1, 2, 3, 5, 8, 12, 19, 20, 21, 25, 40, 41, 60, 79, 80, 85, 100])
    names = []
    while len(names) < size:
        name = rng.choice(NAMES) if rng.random() < 0.5 else ''.join(
            rng.choice('abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_') for _ in range(rng.randint(1, 16)))
        if name.lower() not in {x.lower() for x in names}:
            names.append(name)
    return [{'uuid': str(uuid.UUID(int=rng.getrandbits(128))), 'name': name, 'gameMode': rng.choice([0, 0, 0, 1, 2, 3]),
             'latency': rng.choice([-1, 0, 1, 149, 150, 299, 300, 599, 600, 999, 1000, 5000, rng.randint(0, 2000)]),
             'displayName': component(rng, rng.randint(1, 2)) if rng.random() < 0.2 else None,
             'listOrder': rng.choice([0, 0, 0, 1, -1, 5, 2147483647, -2147483648]),
             'showHat': rng.random() < 0.8, 'inLevel': rng.random() < 0.5} for name in names]


def case(rng):
    players = players_of(rng)
    teams = []
    for t in range(rng.choice([0, 0, 1, 2, 4])):
        teams.append({'name': rng.choice(['red', 'Blue', 'team' + str(t), 'ä', 'z', 'A']) + str(t),
                      'color': rng.choice([None] + COLORS),
                      'prefix': rng.choice(['', '[R] ', {'text': '★ ', 'color': 'gold', 'bold': True}, component(rng, 1)]),
                      'suffix': rng.choice(['', ' ✔', {'text': ' (afk)', 'italic': True}]),
                      'members': [p['name'] for p in players if rng.random() < 0.3]})
    objective = None
    if rng.random() < 0.7:
        objective = {'renderType': rng.choice(['integer', 'hearts', 'hearts']), 'numberFormat': number_format(rng)}
    frames, values, tick = [], {}, rng.randint(0, 100000)
    for f in range(rng.randint(1, 6)):
        scores = {}
        for p in players:
            if objective and (f == 0 and rng.random() < 0.85 or rng.random() < 0.3):
                v = rng.choice([0, 1, 2, 7, 19, 20, 21, 40, 99, 100, -3, 1234567, rng.randint(-50, 300)])
                if p['name'] in values and rng.random() < 0.5:
                    v = values[p['name']] + rng.choice([-3, -1, 1, 2, 10])
                values[p['name']] = v
                scores[p['name']] = {'value': v, 'format': number_format(rng) if rng.random() < 0.2 else None}
        reset = [p['name'] for p in players if objective and rng.random() < 0.03]
        for r in reset:
            values.pop(r, None)
        tick += rng.choice([1, 1, 2, 3, 5, 10, 21, 40])
        frames.append({'tick': tick, 'visible': rng.random() < 0.85, 'scores': scores, 'reset': reset})
    header = footer = None
    if rng.random() < 0.6:
        header = rng.choice([component(rng, rng.randint(1, 30)), 'Line one\nLine two', '\n', 'trailing\n', '  two  spaces  ',
                             {'text': 'Header', 'extra': ['\n', component(rng, 12)]}])
    if rng.random() < 0.5:
        footer = rng.choice([component(rng, rng.randint(1, 30)), 'Footer', 'x\n\ny'])
    return {'width': rng.choice([427, 320, 480, 640, 961, 160, 90, 300]), 'onlineMode': rng.random() < 0.7,
            'chatOnly': rng.random() < 0.8, 'opacity': rng.choice([0.5, 0.0, 1.0, 0.37]),
            'header': header, 'footer': footer, 'teams': teams, 'objective': objective, 'players': players, 'frames': frames}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--count', type=int, default=1000)
    parser.add_argument('--ascii', action='store_true', help="non-ASCII characters but '§' become 'x' (until the probe has the real font)")
    args = parser.parse_args()
    rng = random.Random(0x544142)
    for n in range(args.count):
        c = case(rng)
        for k in range(len(c['frames'])):
            line = json.dumps({**c, 'kind': 'players', 'list': n, 'frames': c['frames'][:k + 1]}, ensure_ascii=False)
            print(''.join('x' if ord(ch) > 127 and ch != '§' else ch for ch in line) if args.ascii else line)


if __name__ == '__main__':
    main()
