#!/usr/bin/env python3
"""Random chat screens, one JSON case per line: a GUI size, the initial text, the sent-message
history, the online players, options, then typed text, keys and clicks, and the mouse for the
frame drawn. `command` cases type into a command tree (`commands`, Tree::from_json's schema) and
also scroll the suggestion list.

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


def lit(name, *children, exec=False):
    return {'literal': name, 'exec': exec, 'children': list(children)}


def arg(name, kind='token', *children, exec=False, **extra):
    return {'argument': name, 'type': kind, 'exec': exec, 'children': list(children), **extra}


TARGETS = ['@a', '@e', '@p', '@r', '@s', 'Steve', 'Alex', 'Player']
ITEMS = ['stone', 'minecraft:stone', 'minecraft:stick', 'diamond_sword', 'minecraft:diamond', 'oak_log', 'Stone_Bricks']
COMMANDS = [
    lit('give', arg('player', 'token', arg('itemName', 'token', arg('amount', 'integer', exec=True, min=1, max=64),
                                           exec=True, suggest=ITEMS), suggest=TARGETS)),
    lit('time', lit('set', arg('time', 'integer', exec=True, min=0)), lit('add', arg('amount', 'integer', exec=True)),
        lit('query', lit('daytime', exec=True), lit('gametime', exec=True), lit('day', exec=True))),
    lit('gamemode', *[lit(m, arg('player', 'token', exec=True, suggest=TARGETS), exec=True)
                      for m in ['survival', 'creative', 'adventure', 'spectator']]),
    lit('tp', arg('x', 'token', arg('y', 'token', arg('z', 'token', exec=True))),
        arg('victim', 'token', arg('destination', 'token', exec=True, suggest=TARGETS), exec=True, suggest=TARGETS)),
    lit('say', arg('message', 'greedy', exec=True)),
    lit('tell', arg('target', 'token', arg('message', 'greedy', exec=True), suggest=TARGETS)),
    lit('w', arg('target', 'token', arg('message', 'greedy', exec=True), suggest=TARGETS)),
    lit('effect', arg('player', 'token', arg('effect', 'token', arg('seconds', 'float', arg('show', 'bool', exec=True),
                                                                     exec=True, min=0.0, max=1e6),
                                         exec=True, suggest=['speed', 'minecraft:haste', 'jump_boost', 'clear']),
                      suggest=TARGETS)),
    lit('gamerule', arg('rule', 'word', arg('value', 'bool', exec=True), exec=True,
                        suggest=['doDaylightCycle', 'keepInventory', 'mobGriefing', 'do_fire_tick'])),
    lit('tag', arg('name', 'string', exec=True)),
    lit('xp', arg('amount', 'integer', exec=True, min=-1000, max=1000)),
    lit('help', exec=True), lit('list', exec=True), lit('me', arg('action', 'greedy', exec=True)),
    lit('kill', arg('target', 'token', exec=True, suggest=TARGETS), exec=True),
    lit('weather', *[lit(w, arg('duration', 'integer', exec=True, min=0), exec=True) for w in ['clear', 'rain', 'thunder']]),
    lit('Tickingarea', lit('add', exec=True), lit('list', exec=True)),
]
COMMAND_TEXTS = ['/', '/g', '/gi', '/give ', '/give S', '/give Steve ', '/give Steve st', '/give Steve stone 5', '/give Steve stone 99',
                 '/give Steve stone x', '/give Steve stone 5 extra', '/time ', '/time q', '/time query ', '/time set -5', '/time set 1000',
                 '/tp 1 2 3', '/tp ~ ~1 ~', '/tp Steve ', '/say hello  there', '/tell Al', '/effect @a speed 1.5 tr',
                 '/effect @s minecraft:', '/effect @s haste 1e', '/effect @s haste -1', '/gamerule keep', '/gamerule mobGriefing f',
                 '/tag "quoted name', '/tag "esc\\q"', '/tag "done" x', '/xp 2000', '/xp -', '/foo', '/foo bar', '/t', '/w ', '/k',
                 '/weather rain 10', '/list extra', '/help', '/Tick', '/gamemode c', '/gamemode creative @', '/ ', '//', ' /give',
                 'not a command', '/me waves', '/tp', '/give ' + 'x' * 60 + ' stone']
COMMAND_KEYS = ['tab', 'tab', 'tab', 'shift+tab', 'up', 'down', 'down', 'left', 'right', 'home', 'end', 'backspace',
                'ctrl+backspace', 'shift+left', 'escape']


def command_case(rng):
    width, height = rng.choice(SIZES)
    actions = [{'type': 'type', 'text': rng.choice(COMMAND_TEXTS)}]
    for _ in range(rng.choice([0, 1, 2, 3, 5, 8])):
        r = rng.random()
        if r < 0.25:
            actions.append({'type': 'type', 'text': rng.choice([' ', 's', 'a', '1', ' 5', 'te', '@', 'q', '"'])})
        elif r < 0.85:
            actions.append({'type': 'key', 'key': rng.choice(COMMAND_KEYS)})
        else:
            actions.append({'type': 'scroll', 'amount': rng.choice([1.0, -1.0, 0.5, -0.5, 3.0, -3.0, -2.0])})
    mouse = rng.choice([(rng.randint(0, width - 1), rng.randint(0, height - 1)),
                        (rng.randint(0, width // 2), rng.randint(height - 15 - 120, height - 15)), (0, 0)])
    return {'width': width, 'height': height, 'initial': rng.choice(['', '', '', '/', '/give ']), 'history': [],
            'players': PLAYERS, 'commands': COMMANDS, 'autoSuggest': rng.random() < 0.8, 'chatOnly': True, 'opacity': 0.5,
            'actions': actions, 'mouse': list(mouse), 'blink': rng.choice([0, 400]), 'kind': 'command'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--count', type=int, default=3000)
    args = parser.parse_args()
    rng = random.Random(0x43484154)
    cases = completion_cases()
    cases += [random_case(rng) for _ in range(max(args.count - len(cases), 0))]
    crng = random.Random(0x434D4453)
    cases += [command_case(crng) for _ in range(args.count // 2)]
    for case in cases:
        print(json.dumps(case, ensure_ascii=False))


if __name__ == '__main__':
    main()
