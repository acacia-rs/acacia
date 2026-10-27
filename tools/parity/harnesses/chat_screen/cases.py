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


def show_text(value):
    return {'action': 'show_text', 'value': value}


HOVERS = [
    show_text('A tooltip'),
    show_text({'text': 'Line one\nLine two', 'color': 'aqua'}),
    show_text('a long tooltip that has to be split because it is much wider than two hundred pixels on any screen at all'),
    show_text({'translate': 'chat.tag.error'}),
    show_text({'text': 'שלום עולם and more', 'bold': True}),
    {'action': 'show_entity', 'id': 'minecraft:pig', 'uuid': '0102030a-0000-4000-8000-00000000abcd', 'name': 'Babe'},
    {'action': 'show_entity', 'id': 'creeper', 'uuid': [1, -2, 3, -4]},
    {'action': 'show_entity', 'id': 'minecraft:villager', 'uuid': 'A1B2C3D4-0001-0002-0003-0000000000FF', 'name': {'text': 'Bob', 'color': 'green'}},
    {'action': 'show_item', 'id': 'minecraft:stone', 'count': 3},
    {'action': 'show_item', 'id': 'minecraft:diamond', 'components': {'minecraft:custom_name': 'Shiny', 'minecraft:lore': ['first lore']}},
]
MESSAGES = [
    lambda h: {'text': 'Hover me', 'hover_event': h},
    lambda h: {'text': 'Hello ', 'extra': [{'text': 'world', 'color': 'gold', 'hover_event': h}, ' and more']},
    lambda h: {'translate': 'chat.type.text', 'with': ['Steve', {'text': 'look here', 'underlined': True, 'hover_event': h}]},
    lambda h: {'text': 'all of it hovers ', 'hover_event': h, 'extra': [{'text': 'even this', 'italic': True}]},
    lambda h: {'text': 'a long message that wraps onto a second line in the chat box because it keeps going and going ' * 2,
               'hover_event': h},
    lambda h: {'text': 'שלום ', 'extra': [{'text': 'עולם', 'hover_event': h}, ' abc']},
    lambda h: 'plain words, no hover',
    lambda h: {'text': '   spaced   out   ', 'hover_event': h},
]
TAGS = [None, None, 'system', 'error', 'not_secure', 'modified']


def hover_case(rng):
    width, height = rng.choice(SIZES)
    advanced = rng.random() < 0.5
    # An advanced item tooltip counts the prototype's components, which the probe has no table of.
    hovers = [h for h in HOVERS if not advanced or h['action'] != 'show_item']
    messages = []
    for _ in range(rng.choice([1, 2, 3, 6, 25])):
        tag = rng.choice(TAGS)
        m = {'text': rng.choice(MESSAGES)(rng.choice(hovers)), 'tag': tag, 'at': 0}
        if tag == 'modified':
            m['original'] = rng.choice(['the original', 'a longer original message that needs to wrap in the tooltip box'])
        messages.append(m)
    scale = rng.choice([1.0, 1.0, 0.5, 0.75])
    bottom = int((height - 40) / scale)
    lines = rng.randint(1, 8)
    mouse = rng.choice([(rng.randint(0, 340), rng.randint(int((bottom - 9 * lines) * scale), height - 38)),
                        (rng.randint(0, 6), rng.randint(int((bottom - 9 * lines) * scale), height - 38)),
                        (rng.randint(0, width - 1), rng.randint(0, height - 1))])
    case = {'width': width, 'height': height, 'initial': '', 'history': [], 'players': PLAYERS, 'autoSuggest': True,
            'chatOnly': True, 'opacity': rng.choice([0.0, 0.5]), 'actions': [], 'mouse': list(mouse), 'blink': 0,
            'kind': 'hover', 'messages': messages, 'now': 10, 'advanced': advanced,
            'chatScale': scale, 'chatWidth': rng.choice([1.0, 0.5]), 'chatOpacity': rng.choice([1.0, 0.4]),
            'chatLineSpacing': rng.choice([0.0, 0.0, 0.5]), 'chatHeightUnfocused': 0.44366195797920227, 'chatHeightFocused': 1.0}
    if len(messages) > 20 and rng.random() < 0.5:
        case['scroll'] = rng.randint(1, 30)
    return case


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--count', type=int, default=3000)
    args = parser.parse_args()
    rng = random.Random(0x43484154)
    cases = completion_cases()
    cases += [random_case(rng) for _ in range(max(args.count - len(cases), 0))]
    crng = random.Random(0x434D4453)
    cases += [command_case(crng) for _ in range(args.count // 2)]
    hrng = random.Random(0x484F5652)
    cases += [hover_case(hrng) for _ in range(args.count // 2)]
    for case in cases:
        print(json.dumps(case, ensure_ascii=False))


if __name__ == '__main__':
    main()
