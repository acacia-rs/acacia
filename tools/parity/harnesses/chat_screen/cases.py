#!/usr/bin/env python3
"""Random chat screens, one JSON case per line: a GUI size, the initial text, the sent-message
history, the online players, options, then typed text, keys and clicks, and the mouse for the
frame drawn. Messages only: no input starts with `/` (commands need the client command tree).

cases.py [--count N]   (Horizon's test_chat_screen_parity.py, MIT, (c) Ludovic Marie)
"""
import argparse
import json
import random

TEXTS = ['hello world', 'a', ' ', 'S', '@', '~ ', 'minecraft:', 'St', 'Al', 'P', 'hi Al', 'tell St', 'p', 'x_', 'b.',
         'hey  there ', 'a long message that goes on and on and on until it no longer fits the chat box at all, really',
         'gg', 'Ünïcödé ★', 'see you at the usual place tomorrow', 'w' * 40, 'q']
KEYS = ['tab', 'tab', 'tab', 'shift+tab', 'up', 'down', 'left', 'right', 'home', 'end', 'backspace', 'backspace',
        'ctrl+backspace', 'delete', 'ctrl+delete', 'ctrl+left', 'ctrl+right', 'shift+left', 'shift+right', 'shift+home',
        'shift+end', 'ctrl+a', 'escape']
HISTORY = ['hi', 'a longer message in the history', 'Steve', 'gg', 'see you tomorrow at the usual place', 'x' * 70]
PLAYERS = ['Player', 'Steve', 'Alex']
MANY = ['Steve', 'Stella', 'stan', 'Stone_Age', 'St.Ive', 'Sam', 'Sally', 'sky_walker', 'Alex', 'alice', 'Al', 'Player',
        'p_l_a_y', 'Bob', 'bobby', 'Zed']
SIZES = [(320, 240), (427, 240), (480, 270), (640, 360), (854, 480), (240, 180)]


def random_case(rng):
    width, height = rng.choice(SIZES)
    actions = []
    for _ in range(rng.choice([1, 2, 3, 5, 8, 12])):
        r = rng.random()
        if r < 0.45:
            actions.append({'type': 'type', 'text': rng.choice(TEXTS)})
        elif r < 0.85:
            key = rng.choice(KEYS)
            if key == 'escape' and rng.random() < 0.7:
                key = 'tab'
            actions.append({'type': 'key', 'key': key})
        else:
            x = rng.randint(0, width // 2)
            y = rng.choice([rng.randint(height - 15 - 120, height - 15), rng.randint(height - 14, height - 1)])
            actions.append({'type': 'click', 'x': x, 'y': y})
    mouse = rng.choice([(rng.randint(0, width - 1), rng.randint(0, height - 1)),
                        (rng.randint(0, width // 2), rng.randint(height - 15 - 120, height - 15)), (0, 0)])
    return {'width': width, 'height': height, 'initial': rng.choice(['', '', '', 'hi ', 'S']),
            'history': rng.sample(HISTORY, rng.randint(0, len(HISTORY))),
            'players': PLAYERS if rng.random() < 0.7 else rng.sample(MANY, rng.randint(0, len(MANY))),
            'autoSuggest': rng.random() < 0.85, 'chatOnly': rng.random() < 0.7, 'opacity': rng.choice([0.0, 0.5, 1.0]),
            'actions': actions, 'mouse': list(mouse), 'blink': rng.choice([0, 100, 400, 650]), 'kind': 'random'}


def completion_cases():
    """Tab through the many-name list (scroll markers, cycling past ten) from a few prefixes."""
    out = []
    for text in ['', 'S', 'hello s', 'a', 'St', 'hi  ', 'x']:
        for keys in [['tab'], ['tab', 'tab'], ['tab'] * 12, ['tab', 'up', 'up'], ['tab', 'down'] + ['down'] * 10,
                     ['tab', 'tab', 'shift+tab'], ['tab', 'escape'], ['tab', 'left', 'tab']]:
            out.append({'width': 640, 'height': 360, 'initial': '', 'history': [], 'players': MANY, 'autoSuggest': True,
                        'chatOnly': True, 'opacity': 0.5, 'mouse': [0, 0], 'blink': 0, 'kind': 'completion',
                        'actions': [{'type': 'type', 'text': text}] + [{'type': 'key', 'key': k} for k in keys]})
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--count', type=int, default=3000)
    args = parser.parse_args()
    rng = random.Random(0x43484154)
    cases = completion_cases()
    cases += [random_case(rng) for _ in range(max(args.count - len(cases), 0))]
    for case in cases:
        print(json.dumps(case, ensure_ascii=False))


if __name__ == '__main__':
    main()
