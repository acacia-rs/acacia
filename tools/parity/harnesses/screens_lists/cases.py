#!/usr/bin/env python3
"""Random key binds, language and pack screen cases, drawn round-robin; one JSON case per line.

cases.py [--count N] [--kind K]... [--seed S]
Every case has the GUI size, the mouse (often over the rows or their buttons), the menu blur and
`focusAgo` (ms since the search box took the focus; kept away from the 300 ms caret flips).
keybinds: `keys` rebinds mappings to ["keyboard"|"mouse", value], `selected` the one waiting for
a key, `warm` draws a frame first, `scroll`. language: `languages` [code, name, region], `selected`,
`search`, `listFocus`, `scroll` (null: where the selection put it). packs: `available` and
`selected` pack lists, `title`, `search`, `scroll` per list, `focus` [list, pack index] or null.
"""
import argparse
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'gui'))
from screencases import message, mouse, size, words  # noqa: E402

MAPPINGS = ['key.attack', 'key.use', 'key.forward', 'key.left', 'key.back', 'key.right', 'key.jump', 'key.sneak', 'key.sprint', 'key.drop',
            'key.inventory', 'key.chat', 'key.playerlist', 'key.pickItem', 'key.command', 'key.friends', 'key.socialInteractions',
            'key.toggleGui', 'key.toggleSpectatorShaderEffects', 'key.screenshot', 'key.togglePerspective', 'key.smoothCamera',
            'key.fullscreen', 'key.spectatorOutlines', 'key.spectatorHotbar', 'key.swapOffhand', 'key.saveToolbarActivator',
            'key.loadToolbarActivator', 'key.advancements', 'key.quickActions', 'key.debug.overlay', 'key.debug.modifier']
MAPPINGS += ['key.hotbar.%d' % n for n in range(1, 10)]
MAPPINGS += ['key.debug.' + n for n in ['reloadChunk', 'showHitboxes', 'clearChat', 'crash', 'showChunkBorders', 'showAdvancedTooltips',
                                        'copyRecreateCommand', 'spectate', 'switchGameMode', 'debugOptions', 'focusPause', 'dumpDynamicTextures',
                                        'reloadResourcePacks', 'profiling', 'copyLocation', 'dumpVersion', 'profilingChart', 'fpsCharts',
                                        'networkCharts', 'lightmapTexture', 'improvedTransparency']]
KEYBOARD = [0, 4, 7, 22, 26, 44, 30, 38, 39, 45, 49, 50, 53, 56, 57, 58, 60, 79, 89, 100, 224, 225, 226, 227, 230, 231, 300, 500]
ICONS = ['misc/unknown_pack', 'gui/title/background/panorama_0', 'gui/presets/isles']
COMPATS = ['COMPATIBLE'] * 5 + ['TOO_OLD', 'TOO_NEW', 'UNKNOWN']


def base(rng):
    case = {**size(rng), 'blur': rng.choice([5, 5, 0, 1, 10]), 'focusAgo': rng.choice([0, 50, 400, 700, 1000])}
    case['mouse'] = mouse(rng, case)
    return case


def keybinds(rng):
    case = base(rng)
    keys = {}
    for name in rng.sample(MAPPINGS, rng.choice([0, 0, 1, 2, 4, 8])):
        keys[name] = ['mouse', rng.randint(1, 9)] if rng.random() < 0.2 else ['keyboard', rng.choice(KEYBOARD)]
    case.update(keys=keys, selected=rng.choice([None, None, rng.choice(MAPPINGS)]), warm=rng.random() < 0.5,
                scroll=rng.choice([0, 0, 15, 100, 333, 700, 10000]))
    if rng.random() < 0.6:
        case['mouse'] = [case['width'] // 2 + rng.randint(-175, 185), rng.randint(20, case['height'])]
    return case


def code(rng):
    return ''.join(rng.choice('abcdefghij') for _ in range(2)) + '_' + ''.join(rng.choice('xyz') for _ in range(2))


def language(rng):
    case = base(rng)
    langs = {code(rng): [words(rng, 1, 2), rng.choice(['US', 'UK', 'Island', 'Wide WWWW region', 'a'])] for _ in range(rng.randint(1, 60))}
    if rng.random() < 0.8:
        langs['en_us'] = ['English', 'US']
    codes = sorted(langs)
    case['languages'] = [[k, *langs[k]] for k in rng.sample(codes, len(codes))]
    case['selected'] = rng.choice(codes + ['en_us', 'zz_zz'])
    case['search'] = rng.choice(['', '', '', 'e', 'the', 'US', 'WWW', 'nothing at all'])
    case['listFocus'] = rng.random() < 0.3
    case['scroll'] = rng.choice([None, None, 0, 40, 300, 5000])
    if rng.random() < 0.6:
        case['mouse'] = [case['width'] // 2 + rng.randint(-140, 140), rng.randint(30, case['height'] - 50)]
    return case


def pack(rng, n, selected):
    p = {'id': rng.choice(['vanilla', 'file/' + words(rng, 1, 1) + '.zip', 'high_contrast', 'fabric']) + str(n),
         'title': rng.choice([message(rng, 1, 3), words(rng, 3, 12), 'Default']),
         'description': rng.choice([message(rng, 0, 20), words(rng, 0, 30), {'text': words(rng, 2, 25), 'color': 'gold'}]),
         'icon': rng.choice(ICONS), 'compat': rng.choice(COMPATS), 'up': rng.random() < 0.5, 'down': rng.random() < 0.5}
    if selected:
        p.update(required=rng.random() < 0.25, fixed=rng.random() < 0.25)
    return p


def packs(rng):
    case = base(rng)
    case['available'] = [pack(rng, n, False) for n in range(rng.choice([0, 1, 3, 6, 12]))]
    case['selected'] = [pack(rng, n, True) for n in range(rng.choice([0, 1, 2, 5, 9]))]
    case['title'] = rng.choice([{'translate': 'resourcePack.title'}, {'translate': 'dataPack.title'}, message(rng)])
    case['search'] = rng.choice(['', '', '', 'a', 'zip', 'the', 'WWW'])
    case['scroll'] = [rng.choice([0, 0, 20, 100, 1000]) for _ in range(2)]
    if rng.random() < 0.35:
        side = rng.randint(0, 1)
        case['focus'] = [side, rng.randint(0, 6)]
    else:
        case['focus'] = None
    if rng.random() < 0.7:
        side = rng.randint(0, 1)
        x = case['width'] // 2 - 15 - 200 if side == 0 else case['width'] // 2 + 15
        row = rng.randint(0, 5)
        case['mouse'] = [x + 4 + rng.randint(-3, 44), 49 + 2 + 13 + 2 + 36 * row + rng.randint(-3, 36)]
    return case


KINDS = dict(keybinds=keybinds, language=language, packs=packs)


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--count', type=int, default=2400)
    p.add_argument('--kind', action='append')
    p.add_argument('--seed', type=int, default=1)
    a = p.parse_args()
    rng = random.Random(a.seed)
    kinds = a.kind or list(KINDS)
    for n in range(a.count):
        kind = kinds[n % len(kinds)]
        print(json.dumps({'kind': kind, **KINDS[kind](rng)}))


if __name__ == '__main__':
    main()
