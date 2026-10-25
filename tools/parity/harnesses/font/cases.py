#!/usr/bin/env python3
"""Font cases, one JSON case per line:

  text     a random styled component (styles, legacy codes, fonts, characters from every bitmap
           sheet and Unicode block, spaces, missing characters) drawn at (x, y) with a colour,
           shadow, background, font options and Font.random seed
  rtl      the same with right-to-left and bidi text (rtl_cases.py)
  visual   such a component's Language.getVisualOrder, character by character
  split    Font.split of such a component at a width: each line's width and characters
  props    ICU's Bidi_Class, mirror and paired bracket type in a range of 4096 codepoints
  glyphs   every glyph of a font in a range of 4096 codepoints, per font option set
  bywidth  a font's glyphsByWidth

cases.py [--count N] [--rtl M] [--seed S] [--kind K]...   N text cases, M of each of rtl, visual, split.
`text` keeps to left-to-right characters; `rtl` covers the rest.
"""
import argparse
import json
import os
from pathlib import Path
import random
import unicodedata

import rtl_cases

ASSETS = Path(os.environ.get('GUI_ASSETS', Path.home() / 'gui-assets' / 'assets' / 'minecraft'))
FONTS = ['minecraft:default', 'minecraft:alt', 'minecraft:uniform', 'minecraft:illageralt', 'minecraft:include/unifont',
         'minecraft:include/default', 'minecraft:include/space', 'minecraft:include/unifont_pua', 'minecraft:nope']
OPTIONS = [(False, False), (True, False), (False, True), (True, True)]
COLORS = ['black', 'dark_blue', 'dark_green', 'dark_aqua', 'dark_red', 'dark_purple', 'gold', 'gray', 'dark_gray', 'blue',
          'green', 'aqua', 'red', 'light_purple', 'yellow', 'white']
CODES = '0123456789abcdefklmnorABCDEFKLMNORzZ '
LTR = {'L', 'EN', 'ES', 'ET', 'CS', 'NSM', 'BN', 'S', 'WS', 'ON'}
# Blocks whose unassigned codepoints are R or AL by default.
RTL_BLOCKS = [(0x0590, 0x08FF), (0xFB1D, 0xFDFF), (0xFE70, 0xFEFF), (0x10800, 0x10FFF), (0x1E800, 0x1EFFF)]


def ltr(cp):
    if 0xD800 <= cp <= 0xDFFF or any(a <= cp <= b for a, b in RTL_BLOCKS):
        return False
    bidi = unicodedata.bidirectional(chr(cp))
    return bidi == '' or bidi in LTR


def sheet_chars():
    chars = set()
    for name in ['default.json', 'include/default.json', 'alt.json', 'illageralt.json']:
        path = ASSETS / 'font' / name
        if path.exists():
            for p in json.loads(path.read_text(encoding='utf-8'))['providers']:
                for row in p.get('chars', []):
                    chars.update(c for c in row if c != '\0' and ltr(ord(c)))
    return sorted(chars)


def character(rng, sheets):
    r = rng.random()
    if r < 0.35:
        cp = rng.randint(0x20, 0x7E)
    elif r < 0.55:
        return rng.choice(sheets)
    elif r < 0.62:
        cp = rng.choice([0x20, 0x20, 0x200C, 0xA0, 0x2003, 0x3000, 0x9, 0xFFFD, 0x3001, 0xAC00, 0x4E00, 0xFF01])
    elif r < 0.9:
        cp = rng.randint(0xA0, 0xFFFF)
    else:
        cp = rng.choice([rng.randint(0x10000, 0x1FFFF), rng.randint(0x20000, 0x3FFFF), rng.randint(0xE0000, 0x10FFFF)])
    return chr(cp) if ltr(cp) else 'x'


def string(rng, sheets):
    out = []
    for _ in range(rng.choice([0, 1, 2, 3, 5, 8, 13, 24])):
        if rng.random() < 0.08:
            out.append('§' + rng.choice(CODES))
        else:
            out.append(character(rng, sheets))
    if rng.random() < 0.03:
        out.append('§')
    return ''.join(out)


def style(rng):
    s = {}
    for k in ['bold', 'italic', 'underlined', 'strikethrough', 'obfuscated']:
        if rng.random() < 0.2:
            s[k] = rng.random() < 0.7
    if rng.random() < 0.3:
        s['color'] = rng.choice(COLORS) if rng.random() < 0.6 else '#%06X' % rng.randint(0, 0xFFFFFF)
    if rng.random() < 0.1:
        s['shadow_color'] = rng.choice([0, -1, 0x7F000000, rng.randint(-2**31, 2**31 - 1)])
    if rng.random() < 0.15:
        s['font'] = rng.choice(FONTS)
    return s


def component(rng, sheets, depth=0):
    c = {'text': string(rng, sheets), **style(rng)}
    if depth < 2 and rng.random() < 0.4:
        c['extra'] = [component(rng, sheets, depth + 1) for _ in range(rng.randint(1, 3))]
    return c


def text(rng, sheets, kind='text'):
    body = component(rng, sheets) if kind == 'text' else rtl_cases.component(rng, style, CODES)
    case = {'kind': kind, 'text': body,
            'x': rng.choice([0, rng.randint(-50, 400), rng.randint(0, 800) / 2, round(rng.uniform(-10, 300), 3)]),
            'y': rng.choice([0, rng.randint(-50, 300), rng.randint(0, 600) / 2]),
            'color': rng.choice([-1, -16777216, rng.randint(-2**31, 2**31 - 1), 0x80FFFFFF - 2**32, 0x00FFFFFF, -5592406]),
            'shadow': rng.random() < 0.6, 'seed': rng.randint(-2**40, 2**40)}
    if rng.random() < 0.1:
        case['bg'] = rng.choice([0x80000000 - 2**32, rng.randint(-2**31, 2**31 - 1)])
    if rng.random() < 0.15:
        case['uniform'], case['jp'] = rng.choice(OPTIONS)
    if rng.random() < 0.05:
        case['fishy'] = True
    return case


def visual(rng):
    return {'kind': 'visual', 'text': rtl_cases.component(rng, style, CODES)}


def split(rng):
    case = {'kind': 'split', 'text': rtl_cases.component(rng, style, CODES),
            'width': rng.choice([1, rng.randint(5, 60), rng.randint(20, 200), rng.randint(100, 400)])}
    if rng.random() < 0.15:
        case['uniform'], case['jp'] = rng.choice(OPTIONS)
    return case


def props():
    for start in range(0, 0x110000, 0x1000):
        yield {'kind': 'props', 'from': start, 'to': start + 0xFFF}


def glyphs():
    for font in FONTS:
        for uniform, jp in OPTIONS if font in ('minecraft:default', 'minecraft:uniform', 'minecraft:include/unifont') else OPTIONS[:1]:
            for start in range(0, 0x110000, 0x1000):
                yield {'kind': 'glyphs', 'font': font, 'from': start, 'to': start + 0xFFF, 'uniform': uniform, 'jp': jp}


def bywidth():
    for font in FONTS:
        for uniform, jp in OPTIONS:
            yield {'kind': 'bywidth', 'font': font, 'uniform': uniform, 'jp': jp}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--count', type=int, default=30000)
    parser.add_argument('--rtl', type=int, default=10000)
    parser.add_argument('--seed', type=int, default=1)
    parser.add_argument('--kind', action='append')
    args = parser.parse_args()
    kinds = set(args.kind or ['text', 'glyphs', 'bywidth', 'rtl', 'visual', 'split', 'props'])
    rng = random.Random(args.seed)
    sheets = sheet_chars()
    cases = []
    if 'bywidth' in kinds:
        cases.extend(bywidth())
    if 'glyphs' in kinds:
        cases.extend(glyphs())
    if 'props' in kinds:
        cases.extend(props())
    if 'text' in kinds:
        cases.extend(text(rng, sheets) for _ in range(args.count))
    # Each kind its own generator, so adding cases of one leaves the others' as they were.
    if 'rtl' in kinds:
        r = random.Random(args.seed * 7 + 1)
        cases.extend(text(r, sheets, 'rtl') for _ in range(args.rtl))
    if 'visual' in kinds:
        r = random.Random(args.seed * 7 + 2)
        cases.extend(visual(r) for _ in range(args.rtl))
    if 'split' in kinds:
        r = random.Random(args.seed * 7 + 3)
        cases.extend(split(r) for _ in range(args.rtl))
    for c in cases:
        print(json.dumps(c))


if __name__ == '__main__':
    main()
